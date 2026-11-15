mod agents;
mod appearance;
mod changes;
mod choice;
mod connection;
mod controls;
mod data;
mod forum;
mod import;
mod landing;
mod links;
mod pages;
mod projects;
mod search;
mod share;
mod sources;
pub(crate) mod widgets;

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AnyView, App, Context, Entity, FocusHandle, Focusable as _, IntoElement, MouseButton, MouseDownEvent, Render,
    ScrollHandle, Window, div, point, prelude::*, px,
};

pub(crate) use self::appearance::found_for as appearance_found;
use self::data::{Described, Script, Setting};
pub(crate) use self::landing::{FoundSetting, parse_found, sheet_of};
use crate::cli;
use crate::panels::PanelKind;
use crate::store::{self, Store, args};
use crate::ui::split::{self, Edge, Split};
use crate::ui::{TextField, TextFieldEvent};

const NAV: Split = Split::new(204., 168., 420.);
const PAGE_KEEP: f32 = 420.;
const SAVED_FOR: Duration = Duration::from_millis(1600);

fn seen_of(store: &Store) -> data::Seen<'_> {
    store
        .session
        .as_ref()
        .map_or(data::Seen { boot: "", revision: 0 }, |s| data::Seen { boot: &s.boot, revision: s.config.revision })
}

fn asked_of(store: &Store) -> data::Asked {
    let seen = seen_of(store);
    data::Asked { boot: seen.boot.to_string(), revision: seen.revision, view: store.view_id }
}

struct NavGrip;

#[derive(Clone, PartialEq, Eq, Hash)]
enum Role {
    Setting(String),
    Secret(String),
    Script(String, Script),
}

struct Failure {
    asked: data::Asked,
    why: String,
}

struct OpenChoice {
    key: String,
    highlight: Option<usize>,
}

pub(crate) struct SettingsPanel {
    described: Option<Described>,
    failed: Option<Failure>,
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
    first_project: Option<u64>,
    card_room: Rc<Cell<f32>>,
    nav_width: Split,
    search: Entity<TextField>,
    scroll: ScrollHandle,
    landed: Option<landing::Landed>,
    landings: u64,
    spot: landing::Spot,
    look: appearance::Look,
}

impl SettingsPanel {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = Store::get(cx);
        cx.observe_in(&store, window, |this, store, window, cx| this.store_changed(&store, window, cx)).detach();
        cx.observe(&crate::appearance::Appearance::get(cx), |_, _, cx| cx.notify()).detach();
        let search = Self::search_field(window, cx);
        window.focus(&search.read(cx).focus_handle(cx));
        let mut this = Self {
            described: None,
            failed: None,
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
            first_project: None,
            card_room: Rc::new(Cell::new(widgets::WIDEST_CARD)),
            nav_width: NAV,
            search,
            scroll: ScrollHandle::new(),
            landed: None,
            landings: 0,
            spot: landing::Spot::default(),
            look: appearance::Look::default(),
        };
        this.open_asked_project(&store, cx);
        this.load(cx);
        this.take_landing(cx);
        this
    }

    fn open_asked_project(&mut self, store: &Entity<Store>, cx: &mut Context<Self>) {
        if let Some(project) = store.update(cx, |s, _| s.take_project_sheet(PanelKind::Settings)) {
            self.first_project = Some(project);
            self.go_to(data::PROJECTS.to_string(), cx);
        }
    }

    fn search_field(window: &mut Window, cx: &mut Context<Self>) -> Entity<TextField> {
        let field = cx.new(|cx| TextField::new("Search settings", cx).clears_on_escape().in_tab_order());
        cx.subscribe_in(&field, window, |this, _, event, _, cx| {
            if matches!(event, TextFieldEvent::Changed) {
                this.search_changed(cx);
            }
        })
        .detach();
        field
    }

    fn close_floating(&mut self, cx: &mut Context<Self>) {
        self.close_choice(cx);
        self.close_picker(cx);
    }

    fn search_changed(&mut self, cx: &mut Context<Self>) {
        self.close_floating(cx);
        self.to_top();
        if !self.query(cx).is_empty() {
            self.landed = None;
        }
        cx.notify();
    }

    fn store_changed(&mut self, store: &Entity<Store>, window: &mut Window, cx: &mut Context<Self>) {
        let s = store.read(cx);
        let open = s.sheet == Some(PanelKind::Settings);
        if open && !self.was_open {
            self.opened(window, cx);
        }
        if open {
            self.take_landing(cx);
        }
        self.open_asked_project(store, cx);
        if open && !self.was_open {
            self.load(cx);
        } else {
            self.read_again(cx);
        }
        if !open {
            self.choice = None;
        }
        self.was_open = open;
    }

    fn opened(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_floating(cx);
        self.landed = None;
        self.first_project = None;
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
        let asked = asked_of(Store::get(cx).read(cx));
        store::run(cx, args(&["config", "--json"]), move |this, result, cx| this.read_ended(asked, result, cx));
    }

    fn read_ended(&mut self, asked: data::Asked, result: anyhow::Result<String>, cx: &mut Context<Self>) {
        self.loading = false;
        let answered = match result.and_then(|out| data::parse(&out)) {
            Ok(described) => {
                self.take(&asked.boot, described, cx);
                true
            }
            Err(e) => {
                self.failed = Some(Failure { asked, why: cli::reason(&e) });
                false
            }
        };
        let revision = self.held_answer().map_or(0, |held| held.revision);
        if let Some(landed) = self.landed.as_mut().filter(|_| answered) {
            landed.take_answer(revision);
        }
        self.read_again(cx);
        cx.notify();
    }

    fn read_again(&mut self, cx: &mut Context<Self>) {
        let store = Store::get(cx).read(cx);
        let sheet = data::Sheet {
            open: store.sheet == Some(PanelKind::Settings),
            reading: self.loading,
            held: self.held_answer(),
            failed: self.failed.as_ref().map(|failure| &failure.asked),
        };
        if data::reads_again(sheet, seen_of(store), store.view_id) {
            self.load(cx);
        }
    }

    fn older(&self, asked: &str, revision: u64) -> bool {
        data::older(self.held_answer(), data::Seen { boot: asked, revision })
    }

    fn take(&mut self, asked: &str, described: Described, cx: &mut Context<Self>) {
        self.failed = None;
        if self.older(asked, described.revision) {
            return;
        }
        asked.clone_into(&mut self.boot);
        self.apply(described, cx);
    }

    fn answer(&self) -> data::Answer<'_> {
        match (&self.described, &self.failed) {
            (Some(described), _) => data::Answer::Held(described),
            (None, Some(failure)) => data::Answer::Failed(&failure.why),
            (None, None) => data::Answer::Reading,
        }
    }

    fn shown(&self, cx: &App) -> data::Shown<'_> {
        data::shown(&self.page, !self.query(cx).is_empty(), self.answer())
    }

    fn held_answer(&self) -> Option<data::Seen<'_>> {
        self.described.as_ref().map(|d| data::Seen { boot: &self.boot, revision: d.revision })
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
        if !pages.contains(&self.page) && !data::OWN_PAGES.contains(&self.page.as_str()) {
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
        for script in Script::all_of(project) {
            let text = script.of(project);
            if text.contains('\n') {
                continue;
            }
            let placeholder = match script {
                Script::Setup => "such as npm ci",
                Script::Teardown => "such as docker compose down",
                Script::Builds => "none",
            };
            self.sync(Role::Script(project.folder.clone(), *script), &text, placeholder, cx);
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
        entity.update(cx, |f, cx| {
            f.set_placeholder(placeholder.to_string(), cx);
            f.keep_saved(value.to_string(), cx);
        });
    }

    fn field(role: Role, placeholder: String, masked: bool, cx: &mut Context<Self>) -> Entity<TextField> {
        let panel = cx.entity();
        let down = role.clone();
        let entity = cx.new(|cx| {
            let field = TextField::new(placeholder, cx).in_tab_order().reverts_on_escape();
            let field =
                field.on_down(move |window, cx| panel.update(cx, |this, cx| this.suggestions_down(&down, window, cx)));
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
        self.close_floating(cx);
        self.landed = None;
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
        self.settle_landing();
        let at = (Edge::Right, self.nav_width.size());
        div()
            .size_full()
            .relative()
            .flex()
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _: &MouseDownEvent, _, cx| this.close_floating(cx)))
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
