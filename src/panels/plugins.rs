mod ask;
mod data;
mod fetch;
mod install;
mod offers;
mod status;
mod window_processes;

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, AnyView, App, AppContext as _, AsyncApp, ClickEvent, Context, Div, Entity, Focusable as _, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, PathPromptOptions, Render, Rgba, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, WeakEntity, Window, div, prelude::FluentBuilder as _, px,
};

use crate::cli;
use crate::dialogs::{self, PathQuestion};
use crate::model::{Session, Shown};
use crate::panels::PanelKind;
use crate::store::{self, Busy, Store, ToastKind, args};
use crate::theme::{colours, radius};
use crate::ui::{self, TextField, TextFieldEvent};
use ask::{Question, text_box};
use data::{List, Plugin, first_line};
use fetch::Checked;
use install::{FETCHING, INSTALLING, LINKING, UPDATING_ALL, checking};
use offers::{Action, target};
use status::{Health, Service};
use window_processes::{Card, WindowProcesses};

#[cfg(test)]
pub(crate) use offers::listed_offers;
pub(crate) use offers::{Button, Group, Offers, Run, Spot, read_offers, run_action};
pub(crate) use window_processes::follow_window_processes;

const LOG_LINES: &str = "60";
const WARNING: &str = "A plugin runs as you and can do anything you can, including typing into every terminal. \
                       What it asks for is shown, not enforced. Plugins start off.";
const NO_PLUGINS: &str = "No plugins yet. Install one from GitHub or a git address above, or link a folder of \
                          yours while you write one.";
const NO_STATUS: &str = "What its services are doing needs the server, which did not answer";
const COUNTS_EVERY: Duration = Duration::from_secs(1);

pub(crate) struct PluginsPanel {
    plugins: Option<Vec<Plugin>>,
    error: Option<String>,
    logs: HashMap<String, String>,
    checks: HashMap<String, Checked>,
    source: Entity<TextField>,
    windows: Entity<WindowProcesses>,
    open: bool,
}

fn shown(cx: &App) -> Shown {
    Store::get(cx).read(cx).session.as_ref().map(Session::shown).unwrap_or_default()
}

fn job(key: &str, label: impl Into<String>) -> Busy {
    Busy::new(format!("plugin:{key}"), label)
}

fn busy(key: &str, cx: &App) -> bool {
    Store::get(cx).read(cx).busy(&format!("plugin:{key}"))
}

fn say(result: anyhow::Result<String>, cx: &mut App) {
    let (text, kind) = match result {
        Ok(text) => (text, ToastKind::Info),
        Err(e) => (e.to_string(), ToastKind::Error),
    };
    if !text.is_empty() {
        Store::get(cx).update(cx, |s, cx| s.toast(text, kind, cx));
    }
}

fn chip(text: impl Into<SharedString>, colour: Rgba) -> Div {
    let text: SharedString = text.into();
    div().px(px(7.)).rounded(px(radius::CHIP)).bg(colours::sidebar()).text_size(px(11.)).text_color(colour).child(text)
}

fn health_colour(health: Health) -> Rgba {
    match health {
        Health::Running => colours::done(),
        Health::Waiting => colours::waiting(),
        Health::Stopped => colours::failed(),
    }
}

fn service_chips(services: &[Service]) -> Vec<Div> {
    services.iter().map(|s| chip(s.describe(), health_colour(s.health()))).collect()
}

fn chips_row(label: &'static str, chips: Vec<Div>) -> Div {
    div().flex().flex_wrap().items_center().gap(px(6.)).child(ui::dim(label)).children(chips)
}

async fn keep_counting(this: WeakEntity<PluginsPanel>, cx: &mut AsyncApp) {
    loop {
        cx.background_executor().timer(COUNTS_EVERY).await;
        if this.update(cx, PluginsPanel::count_down).is_err() {
            return;
        }
    }
}

fn removal(plugin: &Plugin) -> (Question, Vec<String>) {
    let (title, message, button, verb) = if plugin.source.installed() {
        ("Remove", "It is stopped and its installed copy deleted; its settings and data stay.", "Remove", "remove")
    } else {
        ("Unlink", "cornercase forgets it; its folder and its files stay where they are.", "Unlink", "unlink")
    };
    let question = Question {
        title: format!("{title} {}?", plugin.name()),
        message: message.to_string(),
        details: Vec::new(),
        button,
        danger: true,
    };
    (question, args(&["plugin", verb, &plugin.id]))
}

impl PluginsPanel {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let source = cx.new(|cx| {
            TextField::new("owner/repo[/folder][@ref], an https:// git address or a git folder", cx).clears_on_escape()
        });
        cx.subscribe_in(&source, window, |this, _, event, window, cx| match event {
            TextFieldEvent::Submit => this.install(window, cx),
            TextFieldEvent::Changed | TextFieldEvent::Cancel => cx.notify(),
        })
        .detach();
        let store = Store::get(cx);
        cx.observe(&store, |this, store, cx| {
            let open = store.read(cx).sheet == Some(PanelKind::Plugins);
            if open && !this.open {
                this.reload(cx);
            }
            this.open = open;
        })
        .detach();
        let windows = window_processes::get(cx);
        cx.observe(&windows, |_, _, cx| cx.notify()).detach();
        cx.spawn(async move |this, cx| keep_counting(this, cx).await).detach();
        let mut this = Self {
            plugins: None,
            error: None,
            logs: HashMap::new(),
            checks: HashMap::new(),
            source,
            windows,
            open: true,
        };
        this.reload(cx);
        this
    }

    fn count_down(&mut self, cx: &mut Context<Self>) {
        if self.open && self.windows.read(cx).seen.counting() {
            cx.notify();
        }
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let list = if cli::host().is_some() { "status" } else { "list" };
        store::run_json(cx, args(&["plugin", list, "--json"]), |this, result, cx| {
            match result.and_then(|v| serde_json::from_value::<List>(v).map_err(anyhow::Error::from)) {
                Ok(list) => {
                    this.plugins = Some(list.plugins);
                    this.error = None;
                }
                Err(e) => this.error = Some(e.to_string()),
            }
            cx.notify();
        });
        Store::get(cx).update(cx, read_offers);
        if cli::host().is_none() {
            for id in self.logs.keys().cloned().collect::<Vec<_>>() {
                Self::read_log(id, cx);
            }
        }
    }

    fn plugin_cmd(command: Vec<String>, busy: Busy, cx: &mut Context<Self>) {
        Self::plugin_cmd_saying(command, busy, None, cx);
    }

    fn plugin_cmd_saying(command: Vec<String>, busy: Busy, done: Option<String>, cx: &mut Context<Self>) {
        store::run_busy(cx, busy, command, move |this, result, cx| {
            say(result.map(|out| done.unwrap_or_else(|| first_line(&out))), cx);
            this.reload(cx);
        });
    }

    fn switch(plugin: &Plugin, window: &mut Window, cx: &mut Context<Self>) {
        let id = plugin.id.clone();
        if plugin.enabled {
            Self::plugin_cmd(
                args(&["plugin", "disable", &id]),
                job(&id, format!("Turning off {}…", plugin.name())),
                cx,
            );
            return;
        }
        let turning_on = job(&id, format!("Turning on {}…", plugin.name()));
        let question = Question {
            title: format!("Turn on {}?", plugin.name()),
            message: plugin.enable_warning(),
            details: Vec::new(),
            button: "Turn on",
            danger: false,
        };
        let this = cx.entity();
        question.ask(window, cx, move |cx| {
            this.update(cx, |_, cx| Self::plugin_cmd(args(&["plugin", "enable", &id]), turning_on.clone(), cx));
        });
    }

    fn remove(plugin: &Plugin, window: &mut Window, cx: &mut Context<Self>) {
        let removing = job(&plugin.id, format!("Removing {}…", plugin.name()));
        let this = cx.entity();
        let (question, command) = removal(plugin);
        question.ask(window, cx, move |cx| {
            this.update(cx, |_, cx| Self::plugin_cmd(command.clone(), removing.clone(), cx));
        });
    }

    fn link(window: &mut Window, cx: &mut Context<Self>) {
        if let Some(host) = cli::host() {
            let question = PathQuestion::folder(format!("Link a plugin on {host}"), "Link");
            let this = cx.entity();
            let then = move |path: String, _: &mut Window, cx: &mut App| {
                this.update(cx, |_, cx| Self::link_path(&path, cx));
            };
            dialogs::ask_path(question, then, window, cx);
            return;
        }
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Link plugin".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(path) = paths.first()
            {
                let path = path.to_string_lossy().into_owned();
                let _ = this.update(cx, |_, cx| Self::link_path(&path, cx));
            }
        })
        .detach();
    }

    fn link_path(path: &str, cx: &mut Context<Self>) {
        let linking = job(LINKING, format!("Linking {path}…"));
        Self::plugin_cmd(args(&["plugin", "link", path]), linking, cx);
    }

    fn toggle_log(&mut self, id: String, cx: &mut Context<Self>) {
        if self.logs.remove(&id).is_none() {
            self.logs.insert(id.clone(), "…".into());
            Self::read_log(id, cx);
        }
        cx.notify();
    }

    fn read_log(id: String, cx: &mut Context<Self>) {
        store::run(cx, args(&["plugin", "logs", &id, "-n", LOG_LINES]), move |this, result, cx| {
            this.log_read(id, result.unwrap_or_else(|e| e.to_string()), cx);
        });
    }

    fn log_read(&mut self, id: String, text: String, cx: &mut Context<Self>) {
        if let Entry::Occupied(mut e) = self.logs.entry(id) {
            e.insert(if text.trim().is_empty() { "nothing yet".into() } else { text });
            cx.notify();
        }
    }

    fn run_shown(action: Action, cx: &mut Context<Self>) {
        let Some(spot) = target(&action.contexts, shown(cx)) else { return };
        let revision = Store::get(cx).read(cx).plugins.list.revision;
        run_action(&Run::new(action, spot, revision), cx, Self::reload);
    }

    fn plugin_header(plugin: &Plugin, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let busy = busy(&plugin.id, cx);
        let (state, colour) = match (&plugin.error, plugin.enabled) {
            (Some(_), _) => ("cannot run", colours::failed()),
            (None, true) => ("on", colours::done()),
            (None, false) => ("off", colours::faint()),
        };
        let switchable = plugin.error.is_none() && !busy && cli::host().is_none();
        let switched = plugin.clone();
        let chip = if busy {
            ui::spinner(SharedString::from(format!("busy-{}", plugin.id)), 12., colour).into_any_element()
        } else {
            chip(state, colour).into_any_element()
        };
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(
                ui::checkbox(SharedString::from(format!("switch-{}", plugin.id)), plugin.enabled, "")
                    .when(switchable, |d| {
                        d.on_click(
                            cx.listener(move |_, _: &ClickEvent, window, cx| Self::switch(&switched, window, cx)),
                        )
                    })
                    .when(!switchable, |d| d.opacity(0.4).cursor_default()),
            )
            .child(div().font_weight(FontWeight::SEMIBOLD).child(plugin.name()))
            .children(plugin.manifest.as_ref().map(|m| ui::dim(m.version.clone())))
            .child(ui::dim(plugin.id.clone()))
            .child(div().flex_1())
            .child(chip)
    }

    fn services_row(plugin: &Plugin, cx: &App) -> Option<Div> {
        if !plugin.enabled || !plugin.has_service() {
            return None;
        }
        let store = Store::get(cx);
        let Some(status) = &store.read(cx).plugins.status else { return Some(ui::dim(NO_STATUS)) };
        Some(chips_row("Services", service_chips(status.services(&plugin.id))))
    }

    fn window_processes_row(&self, plugin: &Plugin, cx: &App) -> Option<Div> {
        if cli::host().is_some() || !plugin.keeps_window_processes() {
            return None;
        }
        let chips = match self.windows.read(cx).seen.card(&plugin.id, Instant::now()) {
            Card::Starting => vec![chip("starting", colours::waiting())],
            Card::NotRunning(why) => vec![chip(format!("not running, trying again: {why}"), colours::failed())],
            Card::Processes(services) => service_chips(&services),
        };
        Some(chips_row("Window processes", chips))
    }

    fn action_button(action: Action, shown: Shown, cx: &mut Context<Self>) -> AnyElement {
        let key = action.key();
        let id = SharedString::from(format!("action-{key}"));
        if busy(&key, cx) {
            return ui::busy_button(id, action.title, ui::Tone::Plain).into_any_element();
        }
        let runnable = target(&action.contexts, shown).is_some();
        ui::button(id, format!("▶ {}", action.title))
            .when(!runnable, |b| b.opacity(0.45).cursor_default())
            .when(runnable, |b| {
                b.on_click(cx.listener(move |_, _: &ClickEvent, _, cx| Self::run_shown(action.clone(), cx)))
            })
            .into_any_element()
    }

    fn action_buttons(plugin: &Plugin, cx: &mut Context<Self>) -> Option<Div> {
        let store = Store::get(cx);
        let listed = store.read(cx).plugins.list.actions.iter();
        let actions: Vec<Action> = listed.filter(|a| a.plugin == plugin.id).cloned().collect();
        if actions.is_empty() {
            return None;
        }
        let shown = shown(cx);
        let buttons: Vec<AnyElement> =
            actions.into_iter().map(|action| Self::action_button(action, shown, cx)).collect();
        Some(div().flex().flex_wrap().items_center().gap(px(6.)).pt(px(2.)).child(ui::dim("Actions")).children(buttons))
    }

    fn plugin_buttons(&self, plugin: &Plugin, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let id = plugin.id.clone();
        let (log_id, update_id) = (id.clone(), id.clone());
        let removed = plugin.clone();
        let checked = busy(&checking(&id), cx);
        let log_label = if self.logs.contains_key(&id) { "Hide log" } else { "Log" };
        div()
            .flex()
            .gap(px(6.))
            .pt(px(4.))
            .child(
                ui::button(SharedString::from(format!("log-{id}")), log_label)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_log(log_id.clone(), cx))),
            )
            .when(plugin.source.installed(), |d| d.child(Self::update_button(update_id, checked, cx)))
            .child(div().flex_1())
            .child(
                ui::danger_button(
                    SharedString::from(format!("remove-{id}")),
                    if plugin.source.installed() { "Remove" } else { "Unlink" },
                )
                .on_click(cx.listener(move |_, _: &ClickEvent, window, cx| Self::remove(&removed, window, cx))),
            )
    }

    fn update_button(id: String, checked: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let button = SharedString::from(format!("update-{id}"));
        if checked {
            return ui::busy_button(button, "Checking…", ui::Tone::Plain);
        }
        ui::button(button, "Update…").on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
            this.check_updates(vec![id.clone()], &checking(&id), window, cx);
        }))
    }

    fn plugin_card(&self, plugin: &Plugin, cx: &mut Context<Self>) -> AnyElement {
        let description = plugin.manifest.as_ref().and_then(|m| m.description.clone()).filter(|d| !d.is_empty());
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .p(px(14.))
            .rounded(px(radius::CARD))
            .bg(colours::terminal())
            .border_1()
            .border_color(colours::border())
            .child(Self::plugin_header(plugin, cx))
            .children(description.map(|d| div().text_size(px(12.)).text_color(colours::muted()).child(d)))
            .children(plugin.error.clone().map(|e| div().text_size(px(12.)).text_color(colours::failed()).child(e)))
            .child(ui::dim(plugin.facts()))
            .children(Self::services_row(plugin, cx))
            .children(self.window_processes_row(plugin, cx))
            .children(Self::action_buttons(plugin, cx))
            .when(cli::host().is_none(), |d| d.child(self.plugin_buttons(plugin, cx)))
            .children(self.logs.get(&plugin.id).map(|text| log_view(&plugin.id, text)))
            .into_any_element()
    }

    fn install_row(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let focused = self.source.read(cx).focus_handle(cx).is_focused(window);
        let has_source = !self.source.read(cx).text().trim().is_empty();
        let installed = self.installed_ids();
        let checking = busy(UPDATING_ALL, cx);
        div()
            .flex()
            .items_end()
            .gap(px(8.))
            .child(div().flex_1().child(ui::field("Install from", ui::input_box(self.source.clone(), focused))))
            .child(Self::install_button(has_source, cx))
            .child(if busy(LINKING, cx) {
                ui::busy_button("link", "Linking…", ui::Tone::Plain)
            } else {
                ui::button("link", "Link folder…")
                    .on_click(cx.listener(|_, _: &ClickEvent, window, cx| Self::link(window, cx)))
            })
            .when(!installed.is_empty(), |d| d.child(Self::update_all_button(installed, checking, cx)))
            .child(
                ui::icon_button("reload", "↻")
                    .py(px(3.))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.reload(cx))),
            )
    }

    fn installed_ids(&self) -> Vec<String> {
        let plugins = self.plugins.iter().flatten();
        plugins.filter(|p| p.source.installed()).map(|p| p.id.clone()).collect()
    }

    fn install_button(has_source: bool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        if busy(FETCHING, cx) {
            return ui::busy_button("install", "Fetching…", ui::Tone::Primary);
        }
        if busy(INSTALLING, cx) {
            return ui::busy_button("install", "Installing…", ui::Tone::Primary);
        }
        ui::primary_button("install", "Install…")
            .when(!has_source, |b| b.opacity(0.45))
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.install(window, cx)))
    }

    fn update_all_button(ids: Vec<String>, checking: bool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        if checking {
            return ui::busy_button("update-all", "Checking…", ui::Tone::Plain);
        }
        ui::button("update-all", "Update all…").on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
            this.check_updates(ids.clone(), UPDATING_ALL, window, cx);
        }))
    }

    fn problem(cx: &App) -> Option<Div> {
        let problem = Store::get(cx).read(cx).plugins.status.as_ref().and_then(|s| s.problem.clone())?;
        Some(ui::note(problem).text_color(colours::failed()))
    }

    fn list(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        match (&self.plugins, &self.error) {
            (_, Some(e)) => vec![ui::note(e.clone()).text_color(colours::failed()).into_any_element()],
            (None, None) => vec![ui::loading("Reading the plugins…").into_any_element()],
            (Some(ps), None) if ps.is_empty() => vec![ui::note(NO_PLUGINS).into_any_element()],
            (Some(ps), None) => ps.iter().map(|p| self.plugin_card(p, cx)).collect(),
        }
    }
}

fn log_view(id: &str, text: &str) -> impl IntoElement + use<> {
    let lines: Vec<String> = text.lines().map(ToString::to_string).collect();
    text_box(SharedString::from(format!("log-text-{id}")), &lines, 220.)
}

fn remote_note(host: &str) -> String {
    format!(
        "These are the plugins of {host}. Installing, updating, removing, linking, turning them on or off and \
         their logs act on that machine's files, which cornercase does not take through --remote: \
         run `cornercase plugin …` on {host}. Doing it from here needs usecornercase/cornercase-terminal#703."
    )
}

impl Render for PluginsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let install = match cli::host() {
            None => self.install_row(window, cx).into_any_element(),
            Some(host) => ui::dim(remote_note(&host)).into_any_element(),
        };
        let list = self.list(cx);
        div()
            .id("plugins")
            .size_full()
            .overflow_y_scroll()
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(div().text_size(px(12.)).text_color(colours::muted()).child(WARNING))
            .child(install)
            .child(ui::section("Installed"))
            .children(Self::problem(cx))
            .children(list)
    }
}

pub(crate) fn build(window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|cx| PluginsPanel::new(window, cx)).into()
}

#[cfg(test)]
mod tests;
