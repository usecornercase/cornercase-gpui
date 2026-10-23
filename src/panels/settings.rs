//! Every setting, tokens and project commands (`cornercase config`). The form is built from what
//! `config --json` describes (pages, sections, each setting's type, choices and note), so a new
//! setting shows up without a change here; each change is saved at once with `config set`, and the
//! answer, which carries the new revision, replaces what the sheet shows. The session's
//! `config.revision` tells when the settings changed elsewhere (the TUI, an edit of config.json).

mod data;
pub(crate) mod widgets;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use gpui::{
    AnyElement, AnyView, ClipboardItem, Context, Entity, Focusable as _, IntoElement, Render, SharedString, Window, div, prelude::*,
    px,
};

use self::data::{Connection, ConnectionKind, Described, Item, Kind, ProjectEntry, Script, Setting, Tone};
use crate::cli;
use crate::panels::PanelKind;
use crate::store::{self, Store, ToastKind, args};
use crate::theme::colours;
use crate::ui::{self, TextField, TextFieldEvent};

/// What a text field edits.
#[derive(Clone, PartialEq, Eq, Hash)]
enum Role {
    Setting(String),
    /// A tracker's or a bot's token; never pre-filled, cleared once sent.
    Secret(String),
    /// A project's setup or teardown, by folder.
    Script(String, Script),
}

struct Field {
    entity: Entity<TextField>,
    /// The saved value the field was last filled with: the field is edited when its text differs.
    base: String,
}

/// A question asked under a row before something that cannot be undone.
#[derive(Clone, PartialEq, Eq)]
enum Pending {
    Remove(String),
    Choice { key: String, value: String },
    Clear(String, Script),
}

pub struct SettingsPanel {
    described: Option<Described>,
    load_error: Option<String>,
    loading: bool,
    /// The session's boot when the settings were read: a revision only compares within one.
    boot: String,
    page: String,
    fields: HashMap<Role, Field>,
    /// The choice whose list is open.
    open_choice: Option<String>,
    pending: Option<Pending>,
    /// What is being saved or checked, by role.
    busy: HashSet<Role>,
    /// Why a change was refused, by role.
    errors: HashMap<Role, String>,
    exported: Option<data::Exported>,
    was_open: bool,
}

impl SettingsPanel {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
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

    /// Reads the settings again when the sheet opens, and when they changed elsewhere.
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

    /// Shows an answer of `config`: fills the fields that are not being edited.
    fn apply(&mut self, described: Described, cx: &mut Context<Self>) {
        for setting in described.settings.iter().filter(|s| s.edited_as_text()) {
            // An empty value reads as what it means (`per tracker`, `Plane Cloud`), as the TUI shows it.
            let empty = setting.text().is_empty() && !setting.shown.is_empty();
            let placeholder = if empty { setting.shown.clone() } else { setting.label.clone() };
            self.sync(Role::Setting(setting.key.clone()), &setting.text(), &placeholder, cx);
        }
        for connection in described.connections.iter().filter(|c| c.takes_secret()) {
            let role = Role::Secret(connection.key.clone());
            if !self.fields.contains_key(&role) {
                let placeholder = "paste the token, then enter".to_string();
                let entity = self.field(role.clone(), placeholder, true, cx);
                self.fields.insert(role, Field { entity, base: String::new() });
            }
        }
        for project in &described.projects {
            for script in [Script::Setup, Script::Teardown] {
                let text = script.of(project);
                if !text.contains('\n') {
                    let placeholder = match script {
                        Script::Setup => "such as npm ci",
                        Script::Teardown => "such as docker compose down",
                    };
                    self.sync(Role::Script(project.folder.clone(), script), text, placeholder, cx);
                }
            }
        }
        if !data::pages(&described).contains(&self.page) && self.page != data::PROJECTS && self.page != data::SHARE {
            self.page = data::pages(&described).into_iter().next().unwrap_or_else(|| data::PROJECTS.into());
        }
        self.described = Some(described);
        cx.notify();
    }

    /// Makes the field for a role if it has none, and gives it the saved value unless it is being
    /// edited.
    fn sync(&mut self, role: Role, value: &str, placeholder: &str, cx: &mut Context<Self>) {
        match self.fields.get_mut(&role) {
            Some(field) => {
                let edited = field.entity.read(cx).text() != field.base;
                if !edited && field.base != value {
                    field.entity.update(cx, |f, cx| f.set_text(value.to_string(), cx));
                }
                field.base = value.to_string();
            }
            None => {
                let entity = self.field(role.clone(), placeholder.to_string(), false, cx);
                entity.update(cx, |f, cx| f.set_text(value.to_string(), cx));
                self.fields.insert(role, Field { entity, base: value.to_string() });
            }
        }
    }

    fn field(&mut self, role: Role, placeholder: String, masked: bool, cx: &mut Context<Self>) -> Entity<TextField> {
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

    // -----------------------------------------------------------------------------------------
    // Changing things.

    fn submit(&mut self, role: &Role, cx: &mut Context<Self>) {
        let Some(text) = self.fields.get(role).map(|f| f.entity.read(cx).text()) else { return };
        match role {
            Role::Setting(key) => {
                if self.setting(key).is_some_and(|s| s.kind == Kind::Number) && !data::is_number(&text) {
                    self.errors.insert(role.clone(), "a whole number, such as 5".into());
                    cx.notify();
                    return;
                }
                self.save(key.clone(), text.trim().to_string(), cx);
            }
            Role::Secret(key) => self.connect(key.clone(), cx),
            Role::Script(folder, script) => self.save_script(folder.clone(), *script, text, cx),
        }
    }

    /// `config set KEY VALUE`.
    fn save(&mut self, key: String, value: String, cx: &mut Context<Self>) {
        let role = Role::Setting(key.clone());
        self.open_choice = None;
        self.pending = None;
        self.run(role, data::set_args(&key, &value), None, cx);
    }

    fn save_script(&mut self, folder: String, script: Script, text: String, cx: &mut Context<Self>) {
        let Some(entry) = self.described.as_ref().and_then(|d| d.projects.iter().find(|p| p.folder == folder)) else {
            return;
        };
        let command = data::project_args(entry, script, text.trim());
        self.pending = None;
        self.run(Role::Script(folder, script), command, None, cx);
    }

    /// `config secret KEY`, the token on standard input; the field is cleared at once.
    fn connect(&mut self, key: String, cx: &mut Context<Self>) {
        let role = Role::Secret(key.clone());
        let Some(field) = self.fields.get(&role) else { return };
        let token = field.entity.read(cx).text().trim().to_string();
        if token.is_empty() {
            return;
        }
        field.entity.update(cx, |f, cx| f.clear(cx));
        self.run(role, args(&["config", "secret", "--json", "--", &key]), Some(token), cx);
    }

    fn remove(&mut self, key: String, cx: &mut Context<Self>) {
        self.pending = None;
        self.run(Role::Secret(key.clone()), args(&["config", "remove", "--json", "--", &key]), None, cx);
    }

    /// Runs a `config` command that answers like `config --json`, and shows its answer.
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
                // The field no longer counts as edited, so the saved value replaces what was typed.
                if let Some(field) = self.fields.get_mut(&role)
                    && !matches!(role, Role::Secret(_))
                {
                    field.base = field.entity.read(cx).text();
                }
                self.apply(described, cx);
                if !notices.is_empty() {
                    let store = Store::get(cx);
                    store.update(cx, |s, cx| s.toast(notices.join("\n"), ToastKind::Info, cx));
                }
            }
            Err(e) => {
                self.errors.insert(role, data::clean_error(&e));
            }
        }
        cx.notify();
    }

    fn export(&mut self, to: Option<PathBuf>, cx: &mut Context<Self>) {
        let mut command = args(&["config", "export", "--json"]);
        if let Some(path) = &to {
            command.push(format!("--to={}", path.display()));
        }
        store::run(cx, command, move |this, result, cx| {
            let store = Store::get(cx);
            match result.and_then(|out| data::parse_export(&out)) {
                Ok(mut exported) => {
                    let what = format!(
                        "{} settings and {} {}",
                        exported.settings,
                        exported.repositories,
                        if exported.repositories == 1 { "repository" } else { "repositories" }
                    );
                    let text = match &to {
                        Some(path) => format!("Wrote {what} to {}", path.display()),
                        None => {
                            cx.write_to_clipboard(ClipboardItem::new_string(std::mem::take(&mut exported.text)));
                            format!("Copied {what}")
                        }
                    };
                    exported.text.clear();
                    this.exported = Some(exported);
                    store.update(cx, |s, cx| s.toast(text, ToastKind::Done, cx));
                }
                Err(e) => store.update(cx, |s, cx| s.toast(data::clean_error(&e), ToastKind::Error, cx)),
            }
            cx.notify();
        });
    }

    fn export_to_file(&mut self, cx: &mut Context<Self>) {
        let home = std::env::var("HOME").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("/"));
        let rx = cx.prompt_for_new_path(&home, Some("cornercase-setup.toml"));
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(path))) = rx.await {
                let _ = this.update(cx, |this, cx| this.export(Some(path), cx));
            }
        })
        .detach();
    }

    // -----------------------------------------------------------------------------------------
    // Drawing.

    fn render_nav(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let mut pages = self.described.as_ref().map(data::pages).unwrap_or_default();
        let settings_pages = pages.len();
        pages.push(data::PROJECTS.into());
        pages.push(data::SHARE.into());
        let path = self.described.as_ref().map(|d| d.path.clone()).unwrap_or_default();
        let home = std::env::var("HOME").unwrap_or_default();
        let path = match path.strip_prefix(&home) {
            Some(rest) if !home.is_empty() => format!("~{rest}"),
            _ => path,
        };
        widgets::nav()
            .children(pages.into_iter().enumerate().map(|(i, page)| {
                let selected = page == self.page;
                let item = widgets::nav_item(SharedString::from(format!("settings-page-{page}")), page.clone(), selected)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.page = page.clone();
                        this.open_choice = None;
                        this.pending = None;
                        cx.notify();
                    }));
                div().when(i == settings_pages, |d| d.mt(px(8.)).pt(px(8.)).border_t_1().border_color(colours::border())).child(item)
            }))
            .child(div().flex_1())
            .child(ui::dim(path).px(px(8.)))
    }

    fn render_page(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(described) = &self.described else {
            return match &self.load_error {
                Some(e) => ui::note(format!("Could not read the settings: {e}")).into_any_element(),
                None => ui::note("Reading the settings…").into_any_element(),
            };
        };
        let mut page = div().flex().flex_col().gap(px(14.));
        if let Some(problem) = &described.problem {
            page = page.child(
                div()
                    .px(px(12.))
                    .py(px(8.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(colours::failed())
                    .text_color(colours::failed())
                    .child(format!("config.json cannot be read, so nothing is saved until it is fixed: {problem}")),
            );
        }
        match self.page.as_str() {
            data::PROJECTS => page.child(self.render_projects(described, window, cx)).into_any_element(),
            data::SHARE => page.child(self.render_share(cx)).into_any_element(),
            name => {
                page = page.child(widgets::heading(name.to_string(), about(name)));
                for section in data::sections(described, name) {
                    let mut card = widgets::card();
                    for (i, item) in section.items.iter().enumerate() {
                        card = card.child(self.render_item(described, item, i == 0, window, cx));
                    }
                    page = page.child(
                        div()
                            .flex()
                            .flex_col()
                            .when(!section.title.is_empty(), |d| d.child(ui::section(section.title.clone())))
                            .child(card),
                    );
                }
                page.into_any_element()
            }
        }
    }

    fn render_item(&self, d: &Described, item: &Item, first: bool, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        match item {
            Item::Setting(i) => self.render_setting(&d.settings[*i], first, window, cx),
            Item::Connection(i) => self.render_connection(&d.connections[*i], first, window, cx),
            Item::Agent { kind, mode, extra } => {
                let mode = mode.map(|i| &d.settings[i]);
                let extra = extra.map(|i| &d.settings[i]);
                self.render_agent(kind, mode, extra, first, window, cx)
            }
        }
    }

    fn render_setting(&self, setting: &Setting, first: bool, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let role = Role::Setting(setting.key.clone());
        let label = widgets::label(widgets::capitalised(&setting.label), setting.note.clone());
        let control = self.render_control(setting, window, cx);
        let reset = (!setting.is_default() && !setting.default.is_null() && !self.busy.contains(&role)).then(|| {
            let (key, value) = (setting.key.clone(), data::value_text(&setting.default));
            ui::icon_button(SharedString::from(format!("reset-{key}")), "↺")
                .on_click(cx.listener(move |this, _, _, cx| this.save(key.clone(), value.clone(), cx)))
        });
        div()
            .flex()
            .flex_col()
            .child(widgets::row(first, label, div().flex().items_center().gap(px(6.)).child(control).children(reset)))
            .children(self.render_choice_list(setting, cx))
            .children(self.render_pending_choice(setting, cx))
            .children(self.errors.get(&role).map(|e| widgets::error(e.clone())))
            .into_any_element()
    }

    /// The control that changes a setting, by its type.
    fn render_control(&self, setting: &Setting, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let key = setting.key.clone();
        let role = Role::Setting(key.clone());
        if self.busy.contains(&role) && !setting.edited_as_text() {
            return ui::dim("saving…").into_any_element();
        }
        match setting.kind {
            Kind::Switch => {
                let on = setting.on();
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(div().text_size(px(12.)).text_color(colours::muted()).child(setting.switch_text().to_string()))
                    .child(
                        widgets::switch(SharedString::from(format!("switch-{key}")), on)
                            .on_click(cx.listener(move |this, _, _, cx| this.save(key.clone(), (!on).to_string(), cx))),
                    )
                    .into_any_element()
            }
            Kind::Choice if setting.as_segments() => {
                let current = setting.text();
                ui::segments(
                    setting
                        .choices
                        .iter()
                        .map(|choice| {
                            let (key, value) = (key.clone(), choice.value.clone());
                            ui::segment(
                                SharedString::from(format!("choice-{key}-{value}")),
                                choice.value.replace('_', " "),
                                choice.value == current,
                            )
                            .on_click(cx.listener(move |this, _, _, cx| this.choose(key.clone(), value.clone(), cx)))
                            .into_any_element()
                        })
                        .collect(),
                )
                .into_any_element()
            }
            Kind::Choice => self.render_choice_button(setting, cx).into_any_element(),
            Kind::List => self.render_list(setting, cx).into_any_element(),
            Kind::Text | Kind::Number | Kind::Key | Kind::Other => {
                let width = match setting.kind {
                    Kind::Number => 90.,
                    Kind::Key => 160.,
                    _ => 300.,
                };
                self.render_text(&role, width, window, cx).into_any_element()
            }
        }
    }

    /// A text field with Save and Revert while it is edited.
    fn render_text(&self, role: &Role, width: f32, window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let Some(field) = self.fields.get(role) else { return div() };
        let focused = field.entity.read(cx).focus_handle(cx).is_focused(window);
        let edited = self.edited(role, cx);
        let busy = self.busy.contains(role);
        let id = role_id(role);
        let (save, revert) = (role.clone(), role.clone());
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(ui::input_box(field.entity.clone(), focused).w(px(width)).overflow_hidden())
            .when(busy, |d| d.child(ui::dim("saving…")))
            .when(edited && !busy, |d| {
                d.child(
                    ui::button(SharedString::from(format!("revert-{id}")), "Revert")
                        .on_click(cx.listener(move |this, _, _, cx| this.revert(&revert, cx))),
                )
                .child(
                    ui::primary_button(SharedString::from(format!("save-{id}")), "Save")
                        .on_click(cx.listener(move |this, _, _, cx| this.submit(&save, cx))),
                )
            })
    }

    /// Picks a choice; a dangerous one asks first.
    fn choose(&mut self, key: String, value: String, cx: &mut Context<Self>) {
        let dangerous = self.setting(&key).and_then(|s| s.choice(&value)).is_some_and(|c| c.dangerous);
        if dangerous {
            self.pending = Some(Pending::Choice { key, value });
            self.open_choice = None;
            cx.notify();
        } else {
            self.save(key, value, cx);
        }
    }

    fn render_choice_button(&self, setting: &Setting, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let key = setting.key.clone();
        let open = self.open_choice.as_deref() == Some(key.as_str());
        let colour = if setting.dangerous { colours::failed() } else { colours::text() };
        ui::button_coloured(SharedString::from(format!("pick-{key}")), format!("{}  {}", setting.text(), if open { "▴" } else { "▾" }), colour)
            .when(open, |d| d.border_color(colours::accent()))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.open_choice = if this.open_choice.as_deref() == Some(key.as_str()) { None } else { Some(key.clone()) };
                cx.notify();
            }))
    }

    /// The open choice's list, under its row.
    fn render_choice_list(&self, setting: &Setting, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.open_choice.as_deref() != Some(setting.key.as_str()) || setting.as_segments() {
            return None;
        }
        let current = setting.text();
        let rows = setting.choices.iter().map(|choice| {
            let (key, value) = (setting.key.clone(), choice.value.clone());
            let selected = choice.value == current;
            ui::row(SharedString::from(format!("option-{key}-{value}")), selected)
                .child(div().w(px(14.)).flex_none().text_color(colours::accent()).child(if selected { "✓" } else { "" }))
                .child(
                    div()
                        .w(px(220.))
                        .flex_none()
                        .text_color(if choice.dangerous { colours::failed() } else { colours::text() })
                        .child(choice.value.clone()),
                )
                .child(div().flex_1().text_size(px(12.)).text_color(colours::faint()).child(choice.note.clone()))
                .on_click(cx.listener(move |this, _, _, cx| this.choose(key.clone(), value.clone(), cx)))
        });
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(1.))
                .mb(px(9.))
                .p(px(4.))
                .rounded(px(6.))
                .bg(colours::panel())
                .border_1()
                .border_color(colours::border())
                .when_some(setting.question.clone(), |d, q| d.child(ui::dim(q).px(px(8.)).py(px(3.))))
                .children(rows)
                .into_any_element(),
        )
    }

    fn render_pending_choice(&self, setting: &Setting, cx: &mut Context<Self>) -> Option<AnyElement> {
        let Some(Pending::Choice { key, value }) = &self.pending else { return None };
        if *key != setting.key {
            return None;
        }
        let note = setting.choice(value).map(|c| c.note.clone()).unwrap_or_default();
        let (key, value) = (key.clone(), value.clone());
        Some(
            widgets::confirm(
                format!("“{value}” lets agents act without asking ({note}). Use it?"),
                ui::button("pending-cancel", "Cancel").on_click(cx.listener(|this, _, _, cx| {
                    this.pending = None;
                    cx.notify();
                })),
                ui::danger_button("pending-go", "Use it")
                    .on_click(cx.listener(move |this, _, _, cx| this.save(key.clone(), value.clone(), cx))),
            )
            .into_any_element(),
        )
    }

    /// An ordered list (`issue_tabs`): the items in, in order, then the ones that can be added.
    fn render_list(&self, setting: &Setting, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let items = setting.items();
        let key = setting.key.clone();
        let chip = |id: String, label: String, on: bool| {
            ui::button_coloured(SharedString::from(id), label, if on { colours::text() } else { colours::faint() })
                .when(on, |d| d.border_color(colours::accent()))
        };
        let mut row = div().flex().flex_wrap().justify_end().gap(px(4.)).max_w(px(460.));
        for (i, item) in items.iter().enumerate() {
            let (k1, k2, k3, list1, list2, list3, name) =
                (key.clone(), key.clone(), key.clone(), items.clone(), items.clone(), items.clone(), item.clone());
            row = row.child(
                div()
                    .flex()
                    .items_center()
                    .child(
                        ui::icon_button(SharedString::from(format!("left-{key}-{item}")), "‹")
                            .when(i == 0, |d| d.invisible())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.save(k1.clone(), data::list_move(&list1, i, -1).join(","), cx)
                            })),
                    )
                    .child(chip(format!("item-{key}-{item}"), format!("{item} ✕"), true).on_click(cx.listener(
                        move |this, _, _, cx| this.save(k2.clone(), data::list_toggle(&list2, &name).join(","), cx),
                    )))
                    .child(
                        ui::icon_button(SharedString::from(format!("right-{key}-{item}")), "›")
                            .when(i + 1 == items.len(), |d| d.invisible())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.save(k3.clone(), data::list_move(&list3, i, 1).join(","), cx)
                            })),
                    ),
            );
        }
        for choice in setting.choices.iter().filter(|c| !items.contains(&c.value)) {
            let (k, list, name) = (key.clone(), items.clone(), choice.value.clone());
            row = row.child(chip(format!("add-{key}-{name}"), format!("+ {name}"), false).on_click(
                cx.listener(move |this, _, _, cx| this.save(k.clone(), data::list_toggle(&list, &name).join(","), cx)),
            ));
        }
        row
    }

    /// An agent's mode and extra arguments on one row.
    fn render_agent(
        &self,
        kind: &str,
        mode: Option<&Setting>,
        extra: Option<&Setting>,
        first: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mode_control = match mode {
            Some(mode) if self.busy.contains(&Role::Setting(mode.key.clone())) => ui::dim("saving…").into_any_element(),
            Some(mode) => self.render_choice_button(mode, cx).into_any_element(),
            None => ui::dim("no modes").into_any_element(),
        };
        let note = mode.and_then(|m| m.choice(&m.text())).map(|c| c.note.clone()).unwrap_or_default();
        let extra_control = extra.map(|e| self.render_text(&Role::Setting(e.key.clone()), 240., window, cx));
        let roles: Vec<Role> = [mode, extra].into_iter().flatten().map(|s| Role::Setting(s.key.clone())).collect();
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(14.))
                    .py(px(8.))
                    .when(!first, |d| d.border_t_1().border_color(colours::border()))
                    .child(widgets::label(kind.to_string(), note).flex_1().min_w(px(0.)))
                    .child(div().w(px(150.)).flex().justify_end().child(mode_control))
                    .children(extra_control),
            )
            .children(mode.and_then(|m| self.render_choice_list(m, cx)))
            .children(mode.and_then(|m| self.render_pending_choice(m, cx)))
            .children(roles.iter().filter_map(|r| self.errors.get(r)).map(|e| widgets::error(e.clone())))
            .into_any_element()
    }

    fn render_connection(&self, c: &Connection, first: bool, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let role = Role::Secret(c.key.clone());
        let (status, tone) = c.status();
        let colour = match tone {
            Tone::Good => colours::done(),
            Tone::Bad => colours::failed(),
            Tone::Quiet => colours::faint(),
        };
        // The TUI's notes about keys (`enter pastes one`) do not apply here.
        let note = if c.note.starts_with("enter ") { String::new() } else { c.note.clone() };
        let label = div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(div().flex().gap(px(8.)).child(widgets::capitalised(&c.label)).child(
                div().text_size(px(12.)).text_color(colour).child(status),
            ))
            .when(!note.is_empty(), |d| d.child(div().text_size(px(11.5)).text_color(colours::faint()).child(note)))
            .when(c.kind == ConnectionKind::Chat && c.state != data::State::Connected, |d| {
                d.child(
                    div()
                        .text_size(px(11.5))
                        .text_color(colours::faint())
                        .child("pairing shows a code to scan: do it from cornercase's own window, Settings › Remote"),
                )
            });
        let busy = self.busy.contains(&role);
        let key = c.key.clone();
        let mut control = div().flex().items_center().gap(px(6.));
        if busy {
            control = control.child(ui::dim(if c.takes_secret() { "checking…" } else { "saving…" }));
        } else {
            if c.takes_secret()
                && let Some(field) = self.fields.get(&role)
            {
                let focused = field.entity.read(cx).focus_handle(cx).is_focused(window);
                let has_text = !field.entity.read(cx).text().trim().is_empty();
                let k = key.clone();
                control = control.child(ui::input_box(field.entity.clone(), focused).w(px(240.)).overflow_hidden()).when(has_text, |d| {
                    d.child(
                        ui::primary_button(
                            SharedString::from(format!("connect-{key}")),
                            if c.state == data::State::Missing { "Connect" } else { "Replace" },
                        )
                        .on_click(cx.listener(move |this, _, _, cx| this.connect(k.clone(), cx))),
                    )
                });
            }
            if c.removable {
                let k = key.clone();
                let what = if c.kind == ConnectionKind::Browsers { "Forget" } else { "Remove" };
                control = control.child(ui::button(SharedString::from(format!("remove-{key}")), what).on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.pending = Some(Pending::Remove(k.clone()));
                        cx.notify();
                    },
                )));
            }
        }
        let confirm = (self.pending == Some(Pending::Remove(key.clone()))).then(|| {
            let k = key.clone();
            let question = match c.kind {
                ConnectionKind::Browsers => "Forget every paired browser? They need pairing again.".to_string(),
                _ => format!("Remove {}? It has to be set again to use it.", c.label),
            };
            widgets::confirm(
                question,
                ui::button("pending-cancel", "Cancel").on_click(cx.listener(|this, _, _, cx| {
                    this.pending = None;
                    cx.notify();
                })),
                ui::danger_button("pending-go", "Remove").on_click(cx.listener(move |this, _, _, cx| this.remove(k.clone(), cx))),
            )
        });
        div()
            .flex()
            .flex_col()
            .child(widgets::row(first, label, control))
            .children(confirm)
            .children(self.errors.get(&role).map(|e| widgets::error(e.clone())))
            .into_any_element()
    }

    fn render_projects(&self, d: &Described, window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let mut page = div().flex().flex_col().gap(px(14.)).child(widgets::heading(
            "Projects",
            "Commands a repository runs in a new worktree once it is made, and before it is removed.",
        ));
        if d.projects.is_empty() {
            page = page.child(ui::note("No projects: open a repository to give it a setup."));
        }
        for project in &d.projects {
            let name = project.folder.rsplit('/').find(|s| !s.is_empty()).unwrap_or(&project.folder).to_string();
            let mut card = widgets::card().child(
                div()
                    .flex()
                    .items_baseline()
                    .gap(px(8.))
                    .pt(px(10.))
                    .pb(px(4.))
                    .child(ui::title(name))
                    .child(ui::dim(project.folder.clone()))
                    .when(project.project.is_none(), |d| d.child(ui::dim("· not open"))),
            );
            if !project.repository {
                card = card.child(ui::dim("not a git repository: its commands can only be cleared").pb(px(4.)));
            }
            for script in [Script::Setup, Script::Teardown] {
                card = card.child(self.render_script(project, script, window, cx));
            }
            page = page.child(card);
        }
        page
    }

    fn render_script(&self, project: &ProjectEntry, script: Script, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let role = Role::Script(project.folder.clone(), script);
        let text = script.of(project);
        let label = widgets::label(
            widgets::capitalised(script.name()),
            match script {
                Script::Setup => "after a worktree is made, in it",
                Script::Teardown => "before a worktree is removed",
            },
        );
        let control = if text.contains('\n') {
            // A script of several lines does not fit a one-line field: it is shown, and can be cleared.
            let pending = Pending::Clear(project.folder.clone(), script);
            div()
                .flex()
                .items_start()
                .gap(px(6.))
                .child(
                    div()
                        .w(px(300.))
                        .px(px(8.))
                        .py(px(5.))
                        .rounded(px(5.))
                        .bg(colours::panel())
                        .font_family(crate::app::FONT_FAMILY)
                        .text_size(px(12.))
                        .children(text.lines().map(|l| div().child(l.to_string()))),
                )
                .child(ui::button(SharedString::from(format!("clear-{}", role_id(&role))), "Clear").on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.pending = Some(pending.clone());
                        cx.notify();
                    }),
                ))
                .into_any_element()
        } else {
            self.render_text(&role, 300., window, cx).into_any_element()
        };
        let confirm = (self.pending == Some(Pending::Clear(project.folder.clone(), script))).then(|| {
            let folder = project.folder.clone();
            widgets::confirm(
                format!("Clear this {}? Its lines cannot be typed again here.", script.name()),
                ui::button("pending-cancel", "Cancel").on_click(cx.listener(|this, _, _, cx| {
                    this.pending = None;
                    cx.notify();
                })),
                ui::danger_button("pending-go", "Clear")
                    .on_click(cx.listener(move |this, _, _, cx| this.save_script(folder.clone(), script, String::new(), cx))),
            )
        });
        div()
            .flex()
            .flex_col()
            .child(widgets::row(false, label, control))
            .children(confirm)
            .children(self.errors.get(&role).map(|e| widgets::error(e.clone())))
            .into_any_element()
    }

    fn render_share(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let exported = self.exported.clone();
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(widgets::heading(
                "Share",
                "Hand your setup to a colleague: agents, issues, worktrees, the interface and each repository's commands.",
            ))
            .child(
                div().flex().flex_col().child(ui::section("Export")).child(
                    widgets::card()
                        .child(widgets::row(
                            true,
                            widgets::label(
                                "Your setup, as TOML",
                                "never tokens, accounts or personal keys (Jira email, remote access, keep awake)",
                            ),
                            div()
                                .flex()
                                .gap(px(6.))
                                .child(
                                    ui::button("export-file", "Save to file…")
                                        .on_click(cx.listener(|this, _, _, cx| this.export_to_file(cx))),
                                )
                                .child(
                                    ui::primary_button("export-copy", "Copy my setup")
                                        .on_click(cx.listener(|this, _, _, cx| this.export(None, cx))),
                                ),
                        ))
                        .when_some(exported, |d, e| {
                            d.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(3.))
                                    .pb(px(10.))
                                    .child(ui::dim(format!("{} settings and {} repositories went", e.settings, e.repositories)))
                                    .when(!e.left_out.is_empty(), |d| {
                                        d.child(ui::dim(format!("left out: {}", e.left_out.join("; "))))
                                    })
                                    .children(e.warnings.into_iter().map(|w| {
                                        div().text_size(px(12.)).text_color(colours::failed()).child(format!("! {w}"))
                                    })),
                            )
                        }),
                ),
            )
            .child(
                div().flex().flex_col().child(ui::section("Import")).child(
                    widgets::card().child(widgets::row(
                        true,
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(widgets::label(
                                "A setup a colleague shared",
                                "every change, and what would run unattended, has to be shown before it applies",
                            ))
                            .child(widgets::missing("Importing from here waits for a preview of it", 464)),
                        ui::button_coloured("import", "Import…", colours::faint()).cursor_default(),
                    )),
                ),
            )
    }
}

/// What each settings page is about, over its sections.
fn about(page: &str) -> &'static str {
    match page {
        "Worktrees" => "Where worktrees go, how issue branches are named, and fetching.",
        "Agents" => "The agent that starts by default, and how each one starts.",
        "Issues" => "The trackers issues come from, their accounts and filters.",
        "UI" => "How the sidebar, tabs and notifications look and sound.",
        "Remote" => "Reaching this computer's cornercase from a phone or a browser.",
        _ => "",
    }
}

/// A stable id for a field's buttons.
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

pub fn build(window: &mut Window, cx: &mut gpui::App) -> AnyView {
    cx.new(|cx| SettingsPanel::new(window, cx)).into()
}
