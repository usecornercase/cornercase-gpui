use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui::{AnyWindowHandle, App, AppContext as _, Context, Entity, Global, Task, Window};
use serde::{Deserialize, Serialize};

use super::Root;
use super::placement::{self, Placed};
use crate::prefs;
use crate::store::ToastKind;

const FILE: &str = "layout.json";
const WRITE_AFTER: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Saved {
    #[serde(skip_serializing_if = "Widths::is_unset")]
    pub(crate) widths: Widths,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) window: Option<Placed>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Widths {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) sidebar: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) pair: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) panel: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) browser: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) flow: Option<f32>,
}

fn pick(kept: Option<f32>, before: Option<f32>, now: Option<f32>) -> Option<f32> {
    if before == now { kept } else { now }
}

impl Widths {
    pub(crate) fn is_unset(&self) -> bool {
        *self == Self::default()
    }

    pub(crate) fn follow(self, before: Widths, now: Widths) -> Widths {
        Widths {
            sidebar: pick(self.sidebar, before.sidebar, now.sidebar),
            pair: pick(self.pair, before.pair, now.pair),
            panel: pick(self.panel, before.panel, now.panel),
            browser: pick(self.browser, before.browser, now.browser),
            flow: pick(self.flow, before.flow, now.flow),
        }
    }
}

impl Saved {
    pub(crate) fn parse(text: &str) -> Saved {
        serde_json::from_str(text).unwrap_or_default()
    }

    pub(crate) fn text(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).unwrap_or_default();
        text.push('\n');
        text
    }
}

const APART: &str = "window-layout.json";

fn layout_path(settings: &Path) -> PathBuf {
    let beside = prefs::folder_of(settings).join(FILE);
    if beside == settings { settings.with_file_name(APART) } else { beside }
}

pub(crate) struct Keeper {
    path: PathBuf,
    saved: Saved,
    kept: Saved,
    seen: Widths,
    opened: Option<Placed>,
    latest: Option<Placed>,
    window: Option<AnyWindowHandle>,
    writing: Option<Task<()>>,
    failed: Option<String>,
    notices: Vec<String>,
}

struct GlobalKeeper(Entity<Keeper>);
impl Global for GlobalKeeper {}

impl Keeper {
    fn new(path: PathBuf, saved: Saved) -> Keeper {
        Keeper {
            path,
            kept: saved.clone(),
            saved,
            seen: Widths::default(),
            opened: None,
            latest: None,
            window: None,
            writing: None,
            failed: None,
            notices: Vec::new(),
        }
    }

    pub(crate) fn init(cx: &mut App) -> placement::Options {
        let path = layout_path(&prefs::path());
        let saved = std::fs::read_to_string(&path).map(|text| Saved::parse(&text)).unwrap_or_default();
        let options = placement::window_options(saved.window.as_ref(), cx);
        let opened = options.opened.clone();
        let entity = cx.new(|_| {
            let mut keeper = Keeper::new(path, saved);
            keeper.opened_at(opened);
            keeper
        });
        cx.set_global(GlobalKeeper(entity.clone()));
        cx.on_app_quit(move |cx| {
            entity.update(cx, Keeper::flush);
            async {}
        })
        .detach();
        options
    }

    pub(crate) fn get(cx: &App) -> Entity<Keeper> {
        cx.global::<GlobalKeeper>().0.clone()
    }

    pub(crate) fn kept_widths(&self) -> Widths {
        self.kept.widths
    }

    pub(crate) fn seen_widths(&self) -> Widths {
        self.seen
    }

    pub(crate) fn start(&mut self, shown: Widths) {
        self.seen = shown;
    }

    fn opened_at(&mut self, placed: Placed) {
        self.opened = Some(placed);
    }

    pub(crate) fn watch(&mut self, window: AnyWindowHandle) {
        self.window = Some(window);
    }

    fn take_widths(&mut self, now: Widths) -> bool {
        if now == self.seen {
            return false;
        }
        self.kept.widths = self.kept.widths.follow(self.seen, now);
        self.seen = now;
        true
    }

    fn take_window(&mut self, seen: Placed) {
        let before = self.kept.window.as_ref().or(self.opened.as_ref());
        let kept = placement::keep(before, seen);
        if self.kept.window.is_none() && self.opened.as_ref() == Some(&kept) {
            return;
        }
        self.kept.window = Some(kept);
    }

    fn pending(&self) -> Option<String> {
        (self.kept != self.saved).then(|| self.kept.text())
    }

    pub(crate) fn widths_moved(&mut self, now: Widths, cx: &mut Context<Self>) {
        if self.take_widths(now) {
            self.schedule(cx);
        }
    }

    pub(crate) fn window_moved(&mut self, seen: Placed, cx: &mut Context<Self>) {
        self.latest = Some(seen);
        self.schedule(cx);
    }

    fn schedule(&mut self, cx: &mut Context<Self>) {
        self.writing = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(WRITE_AFTER).await;
            this.update(cx, Self::settle).ok();
        }));
    }

    fn apply_window(&mut self, fresh: Option<Placed>) {
        if let Some(seen) = fresh.or_else(|| self.latest.take()) {
            self.take_window(seen);
        }
        self.latest = None;
    }

    fn reread(&mut self, cx: &mut Context<Self>) {
        let fresh = self.window.and_then(|w| w.update(cx, |_, window, cx| placement::seen(window, cx)).ok());
        self.apply_window(fresh);
    }

    fn settle(&mut self, cx: &mut Context<Self>) {
        self.reread(cx);
        match self.write() {
            Ok(()) => self.failed = None,
            Err(why) => {
                let why = format!("{why:#}");
                if self.failed.as_ref() != Some(&why) {
                    self.notices.push(format!("Could not keep the window's layout: {why}"));
                    self.failed = Some(why);
                    cx.notify();
                }
            }
        }
    }

    fn write(&mut self) -> anyhow::Result<()> {
        if let Some(text) = self.pending() {
            prefs::write_atomic(&self.path, &text)?;
            self.saved = self.kept.clone();
        }
        Ok(())
    }

    fn flush(&mut self, cx: &mut Context<Self>) {
        self.writing = None;
        self.reread(cx);
        if let Err(why) = self.write() {
            eprintln!("cornercase-gpui: could not keep the window's layout: {why:#}");
        }
    }

    pub(crate) fn take_notices(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notices)
    }
}

impl Root {
    pub(super) fn follow_window_bounds(window: &mut Window, cx: &mut Context<Self>) {
        Keeper::get(cx).update(cx, |keeper, _| keeper.watch(window.window_handle()));
        cx.observe_window_bounds(window, |_, window, cx| {
            let seen = placement::seen(window, cx);
            Keeper::get(cx).update(cx, |keeper, cx| keeper.window_moved(seen, cx));
        })
        .detach();
    }

    pub(super) fn follow_keeper(cx: &mut Context<Self>) {
        cx.observe(&Keeper::get(cx), |this, keeper, cx| {
            for notice in keeper.update(cx, |keeper, _| keeper.take_notices()) {
                this.store.update(cx, |store, cx| store.toast(notice, ToastKind::Error, cx));
            }
        })
        .detach();
    }
}

#[cfg(test)]
mod tests;
