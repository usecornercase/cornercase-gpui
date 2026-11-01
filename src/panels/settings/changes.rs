use std::path::PathBuf;

use anyhow::Context as _;
use gpui::{AppContext as _, ClipboardItem, Context, Window};

use super::data::{self, Connection, ConnectionKind, Kind, Script};
use super::{Role, SettingsPanel, links};
use crate::cli;
use crate::dialogs::{self, Question};
use crate::store::{self, Busy, Store, ToastKind, args};
use crate::ui::TextField;

pub(super) const EXPORTING: &str = "settings-export";

struct Ask {
    title: String,
    message: String,
    button: &'static str,
}

impl SettingsPanel {
    pub(super) fn submit(&mut self, role: &Role, cx: &mut Context<Self>) {
        let Some(text) = self.fields.get(role).map(|f| f.read(cx).text()) else { return };
        match role {
            Role::Setting(key) => {
                if self.setting(key).is_some_and(|s| s.kind == Kind::Number) && !data::is_number(&text) {
                    self.errors.insert(role.clone(), "Type a whole number, such as 5.".into());
                    cx.notify();
                    return;
                }
                self.save(key, text.trim(), cx);
            }
            Role::Secret(key) => self.connect(key, cx),
            Role::Script(folder, script) => self.save_script(folder, *script, &text, cx),
        }
    }

    pub(super) fn save(&mut self, key: &str, value: &str, cx: &mut Context<Self>) {
        self.choice = None;
        if links::owned(key).is_some() {
            self.save_own(key, value, cx);
            return;
        }
        self.run(Role::Setting(key.to_string()), data::set_args(key, value), None, cx);
    }

    pub(super) fn save_script(&mut self, folder: &str, script: Script, text: &str, cx: &mut Context<Self>) {
        let Some(entry) = self.described.as_ref().and_then(|d| d.projects.iter().find(|p| p.folder == folder)) else {
            return;
        };
        let command = data::project_args(entry, script, text.trim());
        self.run(Role::Script(folder.to_string(), script), command, None, cx);
    }

    pub(super) fn connect(&mut self, key: &str, cx: &mut Context<Self>) {
        let role = Role::Secret(key.to_string());
        let Some(field) = self.fields.get(&role) else { return };
        let token = field.read(cx).text().trim().to_string();
        if token.is_empty() || self.busy.contains(&role) {
            return;
        }
        field.update(cx, TextField::clear);
        self.run(role, args(&["config", "secret", "--json", "--", key]), Some(token), cx);
    }

    fn remove(&mut self, key: &str, cx: &mut Context<Self>) {
        self.run(Role::Secret(key.to_string()), args(&["config", "remove", "--json", "--", key]), None, cx);
    }

    pub(super) fn choose(&mut self, key: String, value: String, window: &mut Window, cx: &mut Context<Self>) {
        self.choice = None;
        let note = self.setting(&key).and_then(|s| s.choice(&value)).filter(|c| c.dangerous).map(|c| c.note.clone());
        let Some(note) = note else {
            self.save(&key, &value, cx);
            return;
        };
        let ask = Ask {
            title: format!("Use “{}”?", data::spoken(&value)),
            message: format!("Agents started this way act without asking you first ({note})."),
            button: "Use it",
        };
        Self::ask(ask, move |this, cx| this.save(&key, &value, cx), window, cx);
    }

    pub(super) fn ask_remove(c: &Connection, window: &mut Window, cx: &mut Context<Self>) {
        let ask = match c.kind {
            ConnectionKind::Browsers => Ask {
                title: "Forget every paired browser?".into(),
                message: "Each one has to be paired again to reach this computer.".into(),
                button: "Forget",
            },
            _ => Ask {
                title: format!("Remove the {}?", c.label),
                message: "cornercase forgets it, and it has to be pasted again to be used.".into(),
                button: "Remove",
            },
        };
        let key = c.key.clone();
        Self::ask(ask, move |this, cx| this.remove(&key, cx), window, cx);
    }

    pub(super) fn ask_clear(folder: &str, script: Script, window: &mut Window, cx: &mut Context<Self>) {
        let ask = Ask {
            title: format!("Clear the {} commands?", script.name()),
            message: "They hold several lines, which cannot be typed again here.".into(),
            button: "Clear",
        };
        let folder = folder.to_string();
        Self::ask(ask, move |this, cx| this.save_script(&folder, script, "", cx), window, cx);
    }

    fn ask(
        ask: Ask,
        then: impl Fn(&mut Self, &mut Context<Self>) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let panel = cx.entity();
        let question = Question::new(ask.title, ask.message, ask.button, Vec::new())
            .on_yes(move |cx| panel.update(cx, |this, cx| then(this, cx)));
        dialogs::confirm(question, window, cx);
        cx.notify();
    }

    fn run(&mut self, role: Role, command: Vec<String>, stdin: Option<String>, cx: &mut Context<Self>) {
        if !self.busy.insert(role.clone()) {
            return;
        }
        self.errors.remove(&role);
        cx.notify();
        let task = cx.background_spawn(async move { cli::run_with_stdin(&command, stdin.as_deref()) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| this.answered(role, result, cx));
        })
        .detach();
    }

    fn answered(&mut self, role: Role, result: anyhow::Result<String>, cx: &mut Context<Self>) {
        self.busy.remove(&role);
        match result.and_then(|out| data::parse(&out)) {
            Ok(described) => {
                let notices = described.notices.clone();
                let field = self.fields.get(&role).filter(|_| !matches!(role, Role::Secret(_))).cloned();
                if let Some(field) = &field {
                    field.update(cx, |f, cx| f.keep_saved(f.text(), cx));
                    self.mark_saved(role, cx);
                }
                self.apply(described, cx);
                if !notices.is_empty() {
                    Store::get(cx).update(cx, |s, cx| s.toast(notices.join("\n"), ToastKind::Info, cx));
                }
            }
            Err(e) => {
                self.errors.insert(role, cli::reason(&e));
            }
        }
        cx.notify();
    }

    pub(super) fn export(to: Option<PathBuf>, cx: &mut Context<Self>) {
        let mut command = args(&["config", "export", "--json"]);
        if let Some(path) = to.as_ref().filter(|_| cli::host().is_none()) {
            command.push(format!("--to={}", path.display()));
        }
        let busy = Busy::new(EXPORTING, "Exporting your setup…");
        store::run_busy(cx, busy, command, move |this, result, cx| {
            match result.and_then(|out| data::parse_export(&out)).and_then(|e| written_here(e, to.as_ref())) {
                Ok(exported) => this.exported_to(exported, to.as_ref(), cx),
                Err(e) => Store::get(cx).update(cx, |s, cx| s.toast(cli::reason(&e), ToastKind::Error, cx)),
            }
            cx.notify();
        });
    }

    fn exported_to(&mut self, mut exported: data::Exported, to: Option<&PathBuf>, cx: &mut Context<Self>) {
        let what = format!(
            "{} settings and {} {}",
            exported.settings,
            exported.repositories,
            if exported.repositories == 1 { "repository" } else { "repositories" }
        );
        let text = if let Some(path) = to {
            format!("Wrote {what} to {}", path.display())
        } else {
            cx.write_to_clipboard(ClipboardItem::new_string(std::mem::take(&mut exported.text)));
            format!("Copied {what}")
        };
        exported.text.clear();
        self.exported = Some(exported);
        Store::get(cx).update(cx, |s, cx| s.toast(text, ToastKind::Done, cx));
    }

    pub(super) fn export_to_file(cx: &mut Context<Self>) {
        let home = std::env::var("HOME").map_or_else(|_| PathBuf::from("/"), PathBuf::from);
        let rx = cx.prompt_for_new_path(&home, Some("cornercase-setup.toml"));
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(path))) = rx.await {
                let _ = this.update(cx, |_, cx| Self::export(Some(path), cx));
            }
        })
        .detach();
    }
}

fn written_here(exported: data::Exported, to: Option<&PathBuf>) -> anyhow::Result<data::Exported> {
    if let Some(path) = to
        && cli::host().is_some()
    {
        std::fs::write(path, &exported.text).with_context(|| format!("could not write {}", path.display()))?;
    }
    Ok(exported)
}
