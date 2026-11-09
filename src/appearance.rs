mod choice;
mod edit;
mod files;
mod watch;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use gpui::{App, AppContext as _, AsyncApp, Context, Entity, Global, WeakEntity, WindowAppearance};

use crate::prefs;
use crate::theme::colour::{hex, opaque};
use crate::theme::current::{Presets, fallback};
use crate::theme::file::{ThemeFile, parse};
use crate::theme::presets::{PRESETS, is_preset};
use crate::theme::resolve::{Lookup, ROOT, Theme, resolve};
use crate::theme::tokens::{OPAQUE, Radius};
use choice::Choice;

pub(crate) use edit::Part;

const POLL: Duration = Duration::from_secs(1);
const SETTLE: Duration = Duration::from_millis(300);

struct Texts<'a>(&'a BTreeMap<String, String>);

impl Lookup for Texts<'_> {
    fn text(&self, id: &str) -> Option<String> {
        Presets.text(id).or_else(|| self.0.get(id).cloned())
    }
}

#[derive(Clone)]
struct Entry {
    theme: Arc<Theme>,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Listed {
    pub id: String,
    pub name: String,
    pub preset: bool,
    pub light: bool,
    pub problem: Option<String>,
}

pub(crate) struct Appearance {
    folder: PathBuf,
    choice: Choice,
    user: BTreeMap<String, String>,
    system_dark: bool,
    resolved: BTreeMap<String, Result<Entry, String>>,
    applied: Option<Arc<Theme>>,
    problem: Option<String>,
    warnings: Vec<String>,
    notices: Vec<String>,
    revision: u64,
    settled: u64,
    seen: watch::Seen,
    pending: BTreeMap<PathBuf, Option<String>>,
}

async fn watching(this: WeakEntity<Appearance>, cx: &mut AsyncApp, folder: PathBuf) {
    loop {
        cx.background_executor().timer(POLL).await;
        let at = folder.clone();
        let seen = cx.background_spawn(async move { watch::snapshot(&at) }).await;
        if this.update(cx, |this, cx| this.poll(seen, cx)).is_err() {
            return;
        }
    }
}

struct GlobalAppearance(Entity<Appearance>);
impl Global for GlobalAppearance {}

pub(crate) fn dark(appearance: WindowAppearance) -> bool {
    matches!(appearance, WindowAppearance::Dark | WindowAppearance::VibrantDark)
}

impl Appearance {
    pub(crate) fn init(cx: &mut App) {
        let folder = files::folder();
        let system_dark = dark(cx.window_appearance());
        let entity = cx.new(|cx| {
            let mut this = Appearance::read(folder, system_dark);
            this.apply(cx);
            this
        });
        cx.set_global(GlobalAppearance(entity.clone()));
        cx.on_app_quit(move |cx| {
            entity.update(cx, |this, _| this.flush());
            async {}
        })
        .detach();
        Self::get(cx).update(cx, |_, cx| Self::watch(cx));
    }

    pub(crate) fn get(cx: &App) -> Entity<Appearance> {
        cx.global::<GlobalAppearance>().0.clone()
    }

    fn read(folder: PathBuf, system_dark: bool) -> Appearance {
        let choice =
            std::fs::read_to_string(files::choice_path(&folder)).map(|t| Choice::parse(&t)).unwrap_or_default();
        let user = files::read_themes(&folder);
        let seen = watch::snapshot(&folder);
        Appearance {
            folder,
            choice,
            user,
            system_dark,
            resolved: BTreeMap::new(),
            applied: None,
            problem: None,
            warnings: Vec::new(),
            notices: Vec::new(),
            revision: 0,
            settled: 0,
            seen,
            pending: BTreeMap::new(),
        }
    }

    fn watch(cx: &mut Context<Self>) {
        let folder = files::folder();
        cx.spawn(async move |this, cx| watching(this, cx, folder).await).detach();
    }

    fn poll(&mut self, seen: watch::Seen, cx: &mut Context<Self>) {
        if seen == self.seen {
            return;
        }
        let paths = watch::changed(&self.seen, &seen);
        self.seen = seen;
        let touched: Vec<bool> = paths.iter().map(|path| self.reload(path)).collect();
        if touched.contains(&true) {
            self.apply(cx);
        }
    }

    fn reload(&mut self, path: &std::path::Path) -> bool {
        if self.pending.contains_key(path) {
            return false;
        }
        let text = std::fs::read_to_string(path).ok();
        if path == files::choice_path(&self.folder) {
            let choice = text.map(|t| Choice::parse(&t));
            return choice.is_some_and(|choice| std::mem::replace(&mut self.choice, choice.clone()) != choice);
        }
        let Some(id) = files::id_of(path) else { return false };
        match text {
            Some(text) => self.user.insert(id, text.clone()) != Some(text),
            None => self.user.remove(&id).is_some(),
        }
    }

    fn rebuild(&mut self) {
        let texts = self.user.clone();
        let lookup = Texts(&texts);
        let ids = PRESETS.iter().map(|(id, _)| (*id).to_string()).chain(texts.keys().cloned());
        self.resolved = ids
            .map(|id| {
                let entry = resolve(&id, &lookup)
                    .map(|r| Entry { theme: Arc::new(r.theme), warnings: r.warnings })
                    .map_err(|e| format!("{e:#}"));
                (id, entry)
            })
            .collect();
        for id in self.user.keys().filter(|id| is_preset(id)) {
            let why = format!("themes/{id}.json is named like a preset: rename it");
            self.resolved.insert(id.clone(), Err(why));
        }
    }

    fn apply(&mut self, cx: &mut Context<Self>) {
        self.rebuild();
        let id = self.choice.in_use(self.system_dark).to_string();
        let found = self.resolved.get(&id).cloned();
        let problem = match found {
            Some(Ok(entry)) => {
                self.warnings = entry.warnings;
                self.applied = Some(entry.theme.clone());
                crate::theme::current::set((*entry.theme).clone());
                None
            }
            Some(Err(why)) => Some(why),
            None => Some(format!("theme.json names {id}, which is gone")),
        };
        self.problem_changed(problem);
        if self.applied.is_none() {
            let theme = fallback();
            self.applied = Some(Arc::new(theme.clone()));
            crate::theme::current::set(theme);
        }
        self.revision += 1;
        cx.refresh_windows();
        cx.notify();
        self.schedule_settle(cx);
    }

    fn problem_changed(&mut self, problem: Option<String>) {
        if problem != self.problem {
            if let Some(why) = &problem {
                self.notices.push(format!("{why}; the window keeps its colours"));
            }
            self.problem = problem;
        }
    }

    fn schedule_settle(&self, cx: &mut Context<Self>) {
        let revision = self.revision;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SETTLE).await;
            this.update(cx, |this, cx| this.settle(revision, cx)).ok();
        })
        .detach();
    }

    fn settle(&mut self, revision: u64, cx: &mut Context<Self>) {
        if revision != self.revision {
            return;
        }
        self.flush();
        self.settled += 1;
        cx.notify();
    }

    fn flush(&mut self) {
        for (path, text) in std::mem::take(&mut self.pending) {
            let done = match &text {
                Some(text) => prefs::write_atomic(&path, text),
                None => std::fs::remove_file(&path).map_err(anyhow::Error::from),
            };
            if let Err(why) = done {
                self.notices.push(format!("could not save a theme: {why:#}"));
            }
        }
    }

    fn save_choice(&mut self) {
        self.pending.insert(files::choice_path(&self.folder), Some(self.choice.text()));
    }

    fn save_theme(&mut self, id: &str, text: String) {
        self.pending.insert(files::theme_path(&self.folder, id), Some(text.clone()));
        self.user.insert(id.to_string(), text);
    }
}

mod api;
mod share;

#[cfg(test)]
mod tests;
