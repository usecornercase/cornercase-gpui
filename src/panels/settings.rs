mod changes;
mod connection;
mod controls;
mod data;
mod pages;
mod projects;
pub(crate) mod widgets;

use std::collections::{HashMap, HashSet};

use gpui::{AnyView, Context, Div, Entity, IntoElement, Render, Stateful, Window, div, prelude::*};

use self::data::{Described, Script, Setting};
use crate::panels::PanelKind;
use crate::store::{self, Store, args};
use crate::ui::{self, TextField, TextFieldEvent};

#[derive(Clone, PartialEq, Eq, Hash)]
enum Role {
    Setting(String),
    Secret(String),
    Script(String, Script),
}

struct Field {
    entity: Entity<TextField>,
    base: String,
}

#[derive(Clone, PartialEq, Eq)]
enum Pending {
    Remove(String),
    Choice { key: String, value: String },
    Clear(String, Script),
}

pub(crate) struct SettingsPanel {
    described: Option<Described>,
    load_error: Option<String>,
    loading: bool,
    boot: String,
    page: String,
    fields: HashMap<Role, Field>,
    open_choice: Option<String>,
    pending: Option<Pending>,
    busy: HashSet<Role>,
    errors: HashMap<Role, String>,
    exported: Option<data::Exported>,
    was_open: bool,
}

impl SettingsPanel {
    pub(crate) fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = Store::get(cx);
        cx.observe(&store, |this, store, cx| this.store_changed(&store, cx)).detach();
        let mut this = Self {
            described: None,
            load_error: None,
            loading: false,
            boot: String::new(),
            page: "Worktrees".into(),
            fields: HashMap::new(),
            open_choice: None,
            pending: None,
            busy: HashSet::new(),
            errors: HashMap::new(),
            exported: None,
            was_open: true,
        };
        this.load(cx);
        this
    }

    fn store_changed(&mut self, store: &Entity<Store>, cx: &mut Context<Self>) {
        let s = store.read(cx);
        let open = s.sheet == Some(PanelKind::Settings);
        let (boot, revision) = s.session.as_ref().map(|s| (s.boot.clone(), s.config.revision)).unwrap_or_default();
        let loaded = self.described.as_ref().map_or(0, |d| d.revision);
        let newer = revision != 0 && (boot != self.boot || revision > loaded);
        if open && (!self.was_open || newer) {
            self.load(cx);
        }
        self.was_open = open;
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.loading = true;
        self.boot = Store::get(cx).read(cx).session.as_ref().map(|s| s.boot.clone()).unwrap_or_default();
        store::run(cx, args(&["config", "--json"]), |this, result, cx| {
            this.loading = false;
            match result.and_then(|out| data::parse(&out)) {
                Ok(described) => {
                    this.load_error = None;
                    this.apply(described, cx);
                }
                Err(e) => this.load_error = Some(data::clean_error(&e)),
            }
            cx.notify();
        });
    }

    fn apply(&mut self, described: Described, cx: &mut Context<Self>) {
        for setting in described.settings.iter().filter(|s| s.edited_as_text()) {
            let empty = setting.text().is_empty() && !setting.shown.is_empty();
            let placeholder = if empty { setting.shown.clone() } else { setting.label.clone() };
            self.sync(Role::Setting(setting.key.clone()), &setting.text(), &placeholder, cx);
        }
        for connection in described.connections.iter().filter(|c| c.takes_secret()) {
            self.add_secret_field(Role::Secret(connection.key.clone()), cx);
        }
        for project in &described.projects {
            self.sync_scripts(project, cx);
        }
        let pages = data::pages(&described);
        if !pages.contains(&self.page) && self.page != data::PROJECTS && self.page != data::SHARE {
            self.page = pages.into_iter().next().unwrap_or_else(|| data::PROJECTS.into());
        }
        self.described = Some(described);
        cx.notify();
    }

    fn add_secret_field(&mut self, role: Role, cx: &mut Context<Self>) {
        if self.fields.contains_key(&role) {
            return;
        }
        let entity = Self::field(role.clone(), "paste the token, then enter".to_string(), true, cx);
        self.fields.insert(role, Field { entity, base: String::new() });
    }

    fn sync_scripts(&mut self, project: &data::ProjectEntry, cx: &mut Context<Self>) {
        for script in [Script::Setup, Script::Teardown] {
            let text = script.of(project);
            if text.contains('\n') {
                continue;
            }
            let placeholder = match script {
                Script::Setup => "such as npm ci",
                Script::Teardown => "such as docker compose down",
            };
            self.sync(Role::Script(project.folder.clone(), script), text, placeholder, cx);
        }
    }

    fn sync(&mut self, role: Role, value: &str, placeholder: &str, cx: &mut Context<Self>) {
        if let Some(field) = self.fields.get_mut(&role) {
            let edited = field.entity.read(cx).text() != field.base;
            if !edited && field.base != value {
                field.entity.update(cx, |f, cx| f.set_text(value.to_string(), cx));
            }
            field.base = value.to_string();
        } else {
            let entity = Self::field(role.clone(), placeholder.to_string(), false, cx);
            entity.update(cx, |f, cx| f.set_text(value.to_string(), cx));
            self.fields.insert(role, Field { entity, base: value.to_string() });
        }
    }

    fn field(role: Role, placeholder: String, masked: bool, cx: &mut Context<Self>) -> Entity<TextField> {
        let entity = cx.new(|cx| {
            let field = TextField::new(placeholder, cx);
            if masked { field.masked() } else { field }
        });
        cx.subscribe(&entity, move |this, _, event: &TextFieldEvent, cx| match event {
            TextFieldEvent::Submit => this.submit(&role, cx),
            TextFieldEvent::Cancel => this.revert(&role, cx),
            TextFieldEvent::Changed => cx.notify(),
        })
        .detach();
        entity
    }

    fn edited(&self, role: &Role, cx: &gpui::App) -> bool {
        self.fields.get(role).is_some_and(|f| f.entity.read(cx).text() != f.base)
    }

    fn revert(&mut self, role: &Role, cx: &mut Context<Self>) {
        if let Some(field) = self.fields.get(role) {
            let base = field.base.clone();
            field.entity.update(cx, |f, cx| f.set_text(base, cx));
        }
        self.errors.remove(role);
        cx.notify();
    }

    fn setting(&self, key: &str) -> Option<&Setting> {
        self.described.as_ref()?.settings.iter().find(|s| s.key == key)
    }

    fn error_of(&self, role: &Role) -> Option<Div> {
        self.errors.get(role).map(|e| widgets::error(e.clone()))
    }

    fn ask(question: impl Into<gpui::SharedString>, go: Stateful<Div>, cx: &mut Context<Self>) -> Div {
        widgets::confirm(
            question,
            ui::button("pending-cancel", "Cancel").on_click(cx.listener(|this, _, _, cx| {
                this.pending = None;
                cx.notify();
            })),
            go,
        )
    }
}

fn role_id(role: &Role) -> String {
    match role {
        Role::Setting(key) => format!("setting-{key}"),
        Role::Secret(key) => format!("secret-{key}"),
        Role::Script(folder, script) => format!("{}-{folder}", script.name()),
    }
}

impl Render for SettingsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .child(self.render_nav(cx))
            .child(widgets::page("settings-page").child(self.render_page(window, cx)))
    }
}

pub(crate) fn build(window: &mut Window, cx: &mut gpui::App) -> AnyView {
    cx.new(|cx| SettingsPanel::new(window, cx)).into()
}
