use std::path::PathBuf;

use gpui::{AppContext as _, ClipboardItem, Context};

use super::data::{self, Kind, Script};
use super::{Pending, Role, SettingsPanel};
use crate::cli;
use crate::store::{self, Store, ToastKind, args};
use crate::ui::TextField;

impl SettingsPanel {
    pub(super) fn submit(&mut self, role: &Role, cx: &mut Context<Self>) {
        let Some(text) = self.fields.get(role).map(|f| f.entity.read(cx).text()) else { return };
        match role {
            Role::Setting(key) => {
                if self.setting(key).is_some_and(|s| s.kind == Kind::Number) && !data::is_number(&text) {
                    self.errors.insert(role.clone(), "a whole number, such as 5".into());
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
        self.open_choice = None;
        self.pending = None;
        self.run(Role::Setting(key.to_string()), data::set_args(key, value), None, cx);
    }

    pub(super) fn save_script(&mut self, folder: &str, script: Script, text: &str, cx: &mut Context<Self>) {
        let Some(entry) = self.described.as_ref().and_then(|d| d.projects.iter().find(|p| p.folder == folder)) else {
            return;
        };
        let command = data::project_args(entry, script, text.trim());
        self.pending = None;
        self.run(Role::Script(folder.to_string(), script), command, None, cx);
    }

    pub(super) fn connect(&mut self, key: &str, cx: &mut Context<Self>) {
        let role = Role::Secret(key.to_string());
        let Some(field) = self.fields.get(&role) else { return };
        let token = field.entity.read(cx).text().trim().to_string();
        if token.is_empty() {
            return;
        }
        field.entity.update(cx, TextField::clear);
        self.run(role, args(&["config", "secret", "--json", "--", key]), Some(token), cx);
    }

    pub(super) fn remove(&mut self, key: &str, cx: &mut Context<Self>) {
        self.pending = None;
        self.run(Role::Secret(key.to_string()), args(&["config", "remove", "--json", "--", key]), None, cx);
    }

    pub(super) fn choose(&mut self, key: String, value: String, cx: &mut Context<Self>) {
        let dangerous = self.setting(&key).and_then(|s| s.choice(&value)).is_some_and(|c| c.dangerous);
        if dangerous {
            self.pending = Some(Pending::Choice { key, value });
            self.open_choice = None;
            cx.notify();
        } else {
            self.save(&key, &value, cx);
        }
    }

    fn run(&mut self, role: Role, command: Vec<String>, stdin: Option<String>, cx: &mut Context<Self>) {
        self.busy.insert(role.clone());
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
                if let Some(field) = self.fields.get_mut(&role)
                    && !matches!(role, Role::Secret(_))
                {
                    field.base = field.entity.read(cx).text();
                }
                self.apply(described, cx);
                if !notices.is_empty() {
                    Store::get(cx).update(cx, |s, cx| s.toast(notices.join("\n"), ToastKind::Info, cx));
                }
            }
            Err(e) => {
                self.errors.insert(role, data::clean_error(&e));
            }
        }
        cx.notify();
    }

    pub(super) fn export(to: Option<PathBuf>, cx: &mut Context<Self>) {
        let mut command = args(&["config", "export", "--json"]);
        if let Some(path) = &to {
            command.push(format!("--to={}", path.display()));
        }
        store::run(cx, command, move |this, result, cx| {
            match result.and_then(|out| data::parse_export(&out)) {
                Ok(exported) => this.exported_to(exported, to.as_ref(), cx),
                Err(e) => Store::get(cx).update(cx, |s, cx| s.toast(data::clean_error(&e), ToastKind::Error, cx)),
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
