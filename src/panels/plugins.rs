mod ask;
mod data;

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

use gpui::{
    AnyElement, AnyView, App, AppContext as _, ClickEvent, Context, Div, Entity, Focusable as _, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, PathPromptOptions, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

use crate::model::{Session, Shown};
use crate::panels::PanelKind;
use crate::store::{self, Store, ToastKind, args};
use crate::theme::colours;
use crate::ui::{self, TextField, TextFieldEvent};
use ask::Question;
use data::{Action, ActionList, List, Plugin, Ran, first_line, shell_quote, target};

const LOG_LINES: &str = "60";
const WARNING: &str = "A plugin runs as you and can do anything you can, including typing into every terminal. \
                       What it asks for is shown, not enforced. Plugins start off.";
const NO_PLUGINS: &str = "No plugins yet. Install one from GitHub or a git address above, or link a folder of \
                          yours while you write one.";
const INSTALL_NOTE: &str = "Installing and updating show what the plugin runs and ask in a terminal tab; doing it \
                            here needs usecornercase/cornercase-terminal#468";
const SERVICE_NOTE: &str = "Whether its service runs needs usecornercase/cornercase-terminal#469";
const POPUP_NOTE: &str =
    "Popup actions open in cornercase's own window; showing them here needs usecornercase/cornercase-terminal#456";

pub(crate) struct PluginsPanel {
    plugins: Option<Vec<Plugin>>,
    error: Option<String>,
    actions: ActionList,
    logs: HashMap<String, String>,
    busy: HashSet<String>,
    source: Entity<TextField>,
    open: bool,
}

fn shown(cx: &App) -> Shown {
    Store::get(cx).read(cx).session.as_ref().map(Session::shown).unwrap_or_default()
}

fn in_a_tab(words: &[&str], name: &str, cx: &mut App) {
    let line =
        std::iter::once("cornercase").chain(words.iter().copied()).map(shell_quote).collect::<Vec<_>>().join(" ");
    Store::get(cx).update(cx, |s, cx| {
        s.act(args(&["new-tab", "--focus", "--name", name, "--", &line]));
        s.close_sheet(cx);
    });
}

fn update(id: Option<&str>, cx: &mut App) {
    let mut words = vec!["plugin", "update"];
    words.extend(id);
    in_a_tab(&words, "update plugin", cx);
}

fn removal(plugin: &Plugin) -> (Question, Vec<String>) {
    let (title, message, button, verb) = if plugin.source.installed() {
        ("Remove", "It is stopped and its installed copy deleted; its settings and data stay.", "Remove", "remove")
    } else {
        ("Unlink", "cornercase forgets it; its folder and its files stay where they are.", "Unlink", "unlink")
    };
    let question =
        Question { title: format!("{title} {}?", plugin.name()), message: message.to_string(), button, danger: true };
    (question, args(&["plugin", verb, &plugin.id]))
}

impl PluginsPanel {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let source =
            cx.new(|cx| TextField::new("owner/repo[/folder][@ref], an https:// git address or a git folder", cx));
        cx.subscribe_in(&source, window, |this, _, event, _, cx| match event {
            TextFieldEvent::Submit => this.install(cx),
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
        let mut this = Self {
            plugins: None,
            error: None,
            actions: ActionList::default(),
            logs: HashMap::new(),
            busy: HashSet::new(),
            source,
            open: true,
        };
        this.reload(cx);
        this
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        store::run_json(cx, args(&["plugin", "list", "--json"]), |this, result, cx| {
            match result.and_then(|v| serde_json::from_value::<List>(v).map_err(anyhow::Error::from)) {
                Ok(list) => {
                    this.plugins = Some(list.plugins);
                    this.error = None;
                }
                Err(e) => this.error = Some(e.to_string()),
            }
            cx.notify();
        });
        store::run_json(cx, args(&["plugin", "actions", "--json"]), |this, result, cx| {
            this.actions = result.ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
            cx.notify();
        });
        for id in self.logs.keys().cloned().collect::<Vec<_>>() {
            Self::read_log(id, cx);
        }
    }

    fn plugin_cmd(&mut self, command: Vec<String>, busy: Option<String>, cx: &mut Context<Self>) {
        if let Some(key) = &busy {
            self.busy.insert(key.clone());
        }
        store::run(cx, command, move |this, result, cx| {
            if let Some(key) = &busy {
                this.busy.remove(key);
            }
            let (text, kind) = match result {
                Ok(out) => (first_line(&out), ToastKind::Info),
                Err(e) => (e.to_string(), ToastKind::Error),
            };
            if !text.is_empty() {
                Store::get(cx).update(cx, |s, cx| s.toast(text, kind, cx));
            }
            this.reload(cx);
        });
    }

    fn switch(&mut self, plugin: &Plugin, window: &mut Window, cx: &mut Context<Self>) {
        let id = plugin.id.clone();
        if plugin.enabled {
            self.plugin_cmd(args(&["plugin", "disable", &id]), Some(id), cx);
            return;
        }
        let question = Question {
            title: format!("Turn on {}?", plugin.name()),
            message: plugin.enable_warning(),
            button: "Turn on",
            danger: false,
        };
        let this = cx.entity();
        question.ask(window, cx, move |cx| {
            this.update(cx, |this, cx| this.plugin_cmd(args(&["plugin", "enable", &id]), Some(id.clone()), cx));
        });
    }

    fn remove(plugin: &Plugin, window: &mut Window, cx: &mut Context<Self>) {
        let id = plugin.id.clone();
        let this = cx.entity();
        let (question, command) = removal(plugin);
        question.ask(window, cx, move |cx| {
            this.update(cx, |this, cx| this.plugin_cmd(command.clone(), Some(id.clone()), cx));
        });
    }

    fn link(cx: &mut Context<Self>) {
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
                let _ = this.update(cx, |this, cx| this.plugin_cmd(args(&["plugin", "link", &path]), None, cx));
            }
        })
        .detach();
    }

    fn install(&mut self, cx: &mut Context<Self>) {
        let source = self.source.read(cx).text().trim().to_string();
        if source.is_empty() {
            return;
        }
        self.source.update(cx, TextField::clear);
        in_a_tab(&["plugin", "install", &source], "install plugin", cx);
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

    fn run_action(&mut self, action: &Action, cx: &mut Context<Self>) {
        let Some(target) = target(&action.contexts, shown(cx)) else { return };
        let key = action.key();
        let mut command = args(&["plugin", "run", &action.plugin, &action.name, "--json"]);
        command.extend(["--revision".to_string(), self.actions.revision.to_string()]);
        command.extend(target);
        self.busy.insert(key.clone());
        cx.notify();
        let title = action.title.clone();
        store::run(cx, command, move |this, result, cx| {
            this.busy.remove(&key);
            let (text, kind) = match result.map(|out| serde_json::from_str::<Ran>(&out)) {
                Ok(Ok(ran)) => ran.say(&title),
                Ok(Err(e)) => (format!("{title}: {e}"), ToastKind::Error),
                Err(e) => (format!("{title}: {e}"), ToastKind::Error),
            };
            Store::get(cx).update(cx, |s, cx| s.toast(text, kind, cx));
            this.reload(cx);
        });
    }

    fn plugin_header(&self, plugin: &Plugin, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let busy = self.busy.contains(&plugin.id);
        let (state, colour) = match (&plugin.error, plugin.enabled) {
            (Some(_), _) => ("cannot run", colours::failed()),
            (None, true) => ("on", colours::done()),
            (None, false) => ("off", colours::faint()),
        };
        let switchable = plugin.error.is_none() && !busy;
        let switched = plugin.clone();
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(
                ui::checkbox(SharedString::from(format!("switch-{}", plugin.id)), plugin.enabled, "")
                    .when(switchable, |d| {
                        d.on_click(
                            cx.listener(move |this, _: &ClickEvent, window, cx| this.switch(&switched, window, cx)),
                        )
                    })
                    .when(!switchable, |d| d.opacity(0.4).cursor_default()),
            )
            .child(div().font_weight(FontWeight::SEMIBOLD).child(plugin.name()))
            .children(plugin.manifest.as_ref().map(|m| ui::dim(m.version.clone())))
            .child(ui::dim(plugin.id.clone()))
            .child(div().flex_1())
            .child(
                div()
                    .px(px(6.))
                    .rounded(px(4.))
                    .bg(colours::sidebar())
                    .text_size(px(11.))
                    .text_color(colour)
                    .child(if busy { "…" } else { state }),
            )
    }

    fn action_button(&self, action: Action, shown: Shown, cx: &mut Context<Self>) -> AnyElement {
        let key = action.key();
        let running = self.busy.contains(&key);
        let runnable = !action.popup() && !running && target(&action.contexts, shown).is_some();
        let label = if running { format!("{} …", action.title) } else { format!("▶ {}", action.title) };
        ui::button(SharedString::from(format!("action-{key}")), label)
            .when(!runnable, |b| b.opacity(0.45).cursor_default())
            .when(runnable, |b| {
                b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.run_action(&action, cx)))
            })
            .into_any_element()
    }

    fn action_buttons(&self, plugin: &Plugin, cx: &mut Context<Self>) -> Option<Div> {
        let actions: Vec<Action> = self.actions.actions.iter().filter(|a| a.plugin == plugin.id).cloned().collect();
        if actions.is_empty() {
            return None;
        }
        let shown = shown(cx);
        let buttons: Vec<AnyElement> =
            actions.into_iter().map(|action| self.action_button(action, shown, cx)).collect();
        Some(div().flex().flex_wrap().items_center().gap(px(6.)).pt(px(2.)).child(ui::dim("Actions")).children(buttons))
    }

    fn plugin_buttons(&self, plugin: &Plugin, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let id = plugin.id.clone();
        let (log_id, update_id) = (id.clone(), id.clone());
        let removed = plugin.clone();
        let log_label = if self.logs.contains_key(&id) { "Hide log" } else { "Log" };
        div()
            .flex()
            .gap(px(6.))
            .pt(px(4.))
            .child(
                ui::button(SharedString::from(format!("log-{id}")), log_label)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_log(log_id.clone(), cx))),
            )
            .when(plugin.source.installed(), |d| {
                d.child(
                    ui::button(SharedString::from(format!("update-{id}")), "Update…")
                        .on_click(move |_: &ClickEvent, _, cx| update(Some(&update_id), cx)),
                )
            })
            .child(div().flex_1())
            .child(
                ui::danger_button(
                    SharedString::from(format!("remove-{id}")),
                    if plugin.source.installed() { "Remove" } else { "Unlink" },
                )
                .on_click(cx.listener(move |_, _: &ClickEvent, window, cx| Self::remove(&removed, window, cx))),
            )
    }

    fn plugin_card(&self, plugin: &Plugin, cx: &mut Context<Self>) -> AnyElement {
        let description = plugin.manifest.as_ref().and_then(|m| m.description.clone()).filter(|d| !d.is_empty());
        let popups = self.actions.actions.iter().any(|a| a.plugin == plugin.id && a.popup());
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .p(px(12.))
            .rounded(px(8.))
            .bg(colours::terminal())
            .border_1()
            .border_color(colours::border())
            .child(self.plugin_header(plugin, cx))
            .children(description.map(|d| div().text_size(px(12.)).text_color(colours::muted()).child(d)))
            .children(plugin.error.clone().map(|e| div().text_size(px(12.)).text_color(colours::failed()).child(e)))
            .child(ui::dim(plugin.facts()))
            .when(plugin.has_service() && plugin.enabled, |d| d.child(ui::dim(SERVICE_NOTE)))
            .children(self.action_buttons(plugin, cx))
            .when(popups, |d| d.child(ui::dim(POPUP_NOTE)))
            .child(self.plugin_buttons(plugin, cx))
            .children(self.logs.get(&plugin.id).map(|text| log_view(&plugin.id, text)))
            .into_any_element()
    }

    fn install_row(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let focused = self.source.read(cx).focus_handle(cx).is_focused(window);
        let has_source = !self.source.read(cx).text().trim().is_empty();
        let installed = self.plugins.as_ref().is_some_and(|ps| ps.iter().any(|p| p.source.installed()));
        div()
            .flex()
            .items_end()
            .gap(px(8.))
            .child(div().flex_1().child(ui::field("Install from", ui::input_box(self.source.clone(), focused))))
            .child(
                ui::primary_button("install", "Install…")
                    .when(!has_source, |b| b.opacity(0.45))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.install(cx))),
            )
            .child(ui::button("link", "Link folder…").on_click(cx.listener(|_, _: &ClickEvent, _, cx| Self::link(cx))))
            .when(installed, |d| {
                d.child(ui::button("update-all", "Update all…").on_click(|_: &ClickEvent, _, cx| update(None, cx)))
            })
            .child(
                ui::icon_button("reload", "↻")
                    .py(px(3.))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.reload(cx))),
            )
    }

    fn list(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        match (&self.plugins, &self.error) {
            (_, Some(e)) => vec![ui::note(e.clone()).text_color(colours::failed()).into_any_element()],
            (None, None) => vec![ui::note("Reading the plugins…").into_any_element()],
            (Some(ps), None) if ps.is_empty() => vec![ui::note(NO_PLUGINS).into_any_element()],
            (Some(ps), None) => ps.iter().map(|p| self.plugin_card(p, cx)).collect(),
        }
    }
}

fn log_view(id: &str, text: &str) -> impl IntoElement + use<> {
    div()
        .id(SharedString::from(format!("log-text-{id}")))
        .max_h(px(220.))
        .overflow_y_scroll()
        .p(px(8.))
        .rounded(px(5.))
        .bg(colours::sidebar())
        .font_family("Menlo")
        .text_size(px(11.))
        .text_color(colours::muted())
        .children(text.lines().map(|l| div().child(l.to_string())).collect::<Vec<_>>())
}

impl Render for PluginsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let install = self.install_row(window, cx);
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
            .child(ui::dim(INSTALL_NOTE))
            .child(ui::section("Installed"))
            .children(list)
    }
}

pub(crate) fn build(window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|cx| PluginsPanel::new(window, cx)).into()
}

#[cfg(test)]
mod tests;
