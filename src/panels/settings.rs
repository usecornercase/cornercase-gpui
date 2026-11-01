mod agents;
mod changes;
mod choice;
mod connection;
mod controls;
mod data;
mod import;
mod links;
mod pages;
mod projects;
mod search;
mod share;
mod sources;
pub(crate) mod widgets;

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use gpui::{
    AnyView, App, Context, Entity, FocusHandle, Focusable as _, IntoElement, MouseButton, MouseDownEvent, Render,
    ScrollHandle, Window, div, point, prelude::*, px,
};

use self::data::{Described, Script, Setting};
use crate::cli;
use crate::panels::PanelKind;
use crate::store::{self, Store, args};
use crate::ui::split::{self, Edge, Split};
use crate::ui::{TextField, TextFieldEvent};

const NAV: Split = Split::new(204., 168., 420.);
const PAGE_KEEP: f32 = 420.;
const SAVED_FOR: Duration = Duration::from_millis(1600);

struct NavGrip;

#[derive(Clone, PartialEq, Eq, Hash)]
enum Role {
    Setting(String),
    Secret(String),
    Script(String, Script),
}

struct OpenChoice {
    key: String,
    highlight: Option<usize>,
}

pub(crate) struct SettingsPanel {
    described: Option<Described>,
    load_error: Option<String>,
    loading: bool,
    boot: String,
    page: String,
    fields: HashMap<Role, Entity<TextField>>,
    choice: Option<OpenChoice>,
    choice_focus: FocusHandle,
    choice_scroll: ScrollHandle,
    busy: HashSet<Role>,
    errors: HashMap<Role, String>,
    saved: Option<Role>,
    exported: Option<data::Exported>,
    import: Option<import::Import>,
    was_open: bool,
    nav_width: Split,
    search: Entity<TextField>,
    scroll: ScrollHandle,
}

impl SettingsPanel {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = Store::get(cx);
        cx.observe_in(&store, window, |this, store, window, cx| this.store_changed(&store, window, cx)).detach();
        let search = Self::search_field(window, cx);
        window.focus(&search.read(cx).focus_handle(cx));
        let mut this = Self {
            described: None,
            load_error: None,
            loading: false,
            boot: String::new(),
            page: "Worktrees".into(),
            fields: HashMap::new(),
            choice: None,
            choice_focus: cx.focus_handle(),
            choice_scroll: ScrollHandle::new(),
            busy: HashSet::new(),
            errors: HashMap::new(),
            saved: None,
            exported: None,
            import: None,
            was_open: true,
            nav_width: NAV,
            search,
            scroll: ScrollHandle::new(),
        };
        this.load(cx);
        this
    }

    fn search_field(window: &mut Window, cx: &mut Context<Self>) -> Entity<TextField> {
        let field = cx.new(|cx| TextField::new("Search settings", cx).clears_on_escape().in_tab_order());
        cx.subscribe_in(&field, window, |this, _, event, _, cx| {
            if matches!(event, TextFieldEvent::Changed) {
                this.choice = None;
                this.to_top();
                cx.notify();
            }
        })
        .detach();
        field
    }

    fn store_changed(&mut self, store: &Entity<Store>, window: &mut Window, cx: &mut Context<Self>) {
        let s = store.read(cx);
        let open = s.sheet == Some(PanelKind::Settings);
        let (boot, revision) = s.session.as_ref().map(|s| (s.boot.clone(), s.config.revision)).unwrap_or_default();
        let loaded = self.described.as_ref().map_or(0, |d| d.revision);
        let newer = revision != 0 && (boot != self.boot || revision > loaded);
        if open && !self.was_open {
            self.opened(window, cx);
        }
        if open && (!self.was_open || newer) {
            self.load(cx);
        }
        if !open {
            self.choice = None;
        }
        self.was_open = open;
    }

    fn opened(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.choice = None;
        self.search.update(cx, TextField::clear);
        window.focus(&self.search.read(cx).focus_handle(cx));
        self.to_top();
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.loading = true;
        cx.notify();
        self.boot = Store::get(cx).read(cx).session.as_ref().map(|s| s.boot.clone()).unwrap_or_default();
        store::run(cx, args(&["config", "--json"]), |this, result, cx| {
            this.loading = false;
            match result.and_then(|out| data::parse(&out)) {
                Ok(described) => {
                    this.load_error = None;
                    this.apply(described, cx);
                }
                Err(e) => this.load_error = Some(cli::reason(&e)),
            }
            cx.notify();
        });
    }

    fn apply(&mut self, mut described: Described, cx: &mut Context<Self>) {
        links::add(&mut described, &Store::get(cx).read(cx).prefs);
        for setting in described.settings.iter().filter(|s| s.edited_as_text()) {
            let placeholder = data::placeholder(setting);
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
        let entity = Self::field(role.clone(), "paste the token".to_string(), true, cx);
        self.fields.insert(role, entity);
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
        let entity = if let Some(entity) = self.fields.get(&role) {
            entity.clone()
        } else {
            let entity = Self::field(role.clone(), placeholder.to_string(), false, cx);
            self.fields.insert(role, entity.clone());
            entity
        };
        entity.update(cx, |f, cx| f.keep_saved(value.to_string(), cx));
    }

    fn field(role: Role, placeholder: String, masked: bool, cx: &mut Context<Self>) -> Entity<TextField> {
        let entity = cx.new(|cx| {
            let field = TextField::new(placeholder, cx).in_tab_order().reverts_on_escape();
            if masked { field.masked() } else { field }
        });
        cx.subscribe(&entity, move |this, _, event: &TextFieldEvent, cx| match event {
            TextFieldEvent::Submit => this.submit(&role, cx),
            TextFieldEvent::Cancel => {
                this.errors.remove(&role);
                cx.notify();
            }
            TextFieldEvent::Changed => cx.notify(),
        })
        .detach();
        entity
    }

    fn edited(&self, role: &Role, cx: &App) -> bool {
        self.fields.get(role).is_some_and(|f| f.read(cx).edited())
    }

    fn revert(&mut self, role: &Role, cx: &mut Context<Self>) {
        if let Some(field) = self.fields.get(role) {
            field.update(cx, TextField::revert);
        }
        self.errors.remove(role);
        cx.notify();
    }

    fn setting(&self, key: &str) -> Option<&Setting> {
        self.described.as_ref()?.settings.iter().find(|s| s.key == key)
    }

    fn query(&self, cx: &App) -> String {
        self.search.read(cx).text().trim().to_string()
    }

    fn go_to(&mut self, page: String, cx: &mut Context<Self>) {
        self.page = page;
        self.choice = None;
        self.search.update(cx, TextField::clear);
        self.to_top();
        cx.notify();
    }

    fn to_top(&self) {
        self.scroll.set_offset(point(px(0.), px(0.)));
    }

    fn mark_saved(&mut self, role: Role, cx: &mut Context<Self>) {
        self.saved = Some(role.clone());
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SAVED_FOR).await;
            let _ = this.update(cx, |this, cx| this.unmark_saved(&role, cx));
        })
        .detach();
    }

    fn unmark_saved(&mut self, role: &Role, cx: &mut Context<Self>) {
        if self.saved.as_ref() == Some(role) {
            self.saved = None;
            cx.notify();
        }
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
        let at = (Edge::Right, self.nav_width.size());
        div()
            .size_full()
            .relative()
            .flex()
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _: &MouseDownEvent, _, cx| this.close_choice(cx)))
            .on_drag_move(split::follow::<_, NavGrip>(Edge::Right, PAGE_KEEP, nav_width, cx))
            .child(self.render_nav(window, cx))
            .child(widgets::page("settings-page", &self.scroll).child(self.render_page(window, cx)))
            .child(split::handle("settings-split", NavGrip, at, nav_width, cx))
    }
}

fn nav_width(panel: &mut SettingsPanel) -> &mut Split {
    &mut panel.nav_width
}

pub(crate) fn build(window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|cx| SettingsPanel::new(window, cx)).into()
}
