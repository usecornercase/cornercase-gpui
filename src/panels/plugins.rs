//! The plugins that extend cornercase (`cornercase plugin`): what is installed or linked, whether
//! it is on, what it asks for and runs, its actions (run on what the window shows) and its log;
//! turning each on and off, linking a folder, installing, updating and removing. Like the TUI's
//! Settings › Plugins, with no catalogue: finding plugins is the website.

use std::collections::{HashMap, HashSet};

use gpui::{
    AnyView, App, AppContext as _, ClickEvent, Context, Entity, FocusHandle, Focusable as _, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, PathPromptOptions, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _, px,
};
use serde::Deserialize;
use serde_json::Value;

use crate::dialogs;
use crate::model::Shown;
use crate::panels::PanelKind;
use crate::store::{self, Store, ToastKind, args};
use crate::theme::colours;
use crate::ui::{self, TextField, TextFieldEvent};

/// How many lines of a plugin's log the sheet shows.
const LOG_LINES: &str = "60";

pub(crate) struct PluginsPanel {
    plugins: Option<Vec<Plugin>>,
    /// Why `plugin list` failed, if it did.
    error: Option<String>,
    actions: Actions,
    /// Plugins whose log is shown, with what it said.
    logs: HashMap<String, String>,
    /// Actions running now (`plugin/action`), and plugins being switched.
    busy: HashSet<String>,
    source: Entity<TextField>,
    /// Whether the sheet is showing, to read the plugins again each time it opens.
    open: bool,
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
            actions: Actions::default(),
            logs: HashMap::new(),
            busy: HashSet::new(),
            source,
            open: true,
        };
        this.reload(cx);
        this
    }

    /// Reads the plugins (`plugin list`, from the files) and their actions (from the server).
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
            // Without a server (or an older one) there are no actions to offer; the list stays.
            this.actions = result.ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
            cx.notify();
        });
        let shown: Vec<String> = self.logs.keys().cloned().collect();
        for id in shown {
            self.read_log(id, cx);
        }
    }

    /// Runs a `cornercase plugin …` command, says how it went and reads the plugins again.
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
        // Turning one on is the explicit step: say plainly what that means first.
        let asks = plugin.manifest.as_ref().map(Manifest::asks).unwrap_or_default();
        let runs = plugin.manifest.as_ref().map(Manifest::runs).unwrap_or_default();
        let mut message =
            format!("{} runs as you and can do anything you can, including typing into every terminal.", plugin.name());
        message.push_str(&format!(" It {asks}; that is shown, not enforced."));
        if !runs.is_empty() {
            message.push_str(&format!(" It runs {}.", runs.join(", ")));
        }
        let this = cx.entity();
        ask(format!("Turn on {}?", plugin.name()), message, "Turn on", false, window, cx, move |cx| {
            this.update(cx, |this, cx| this.plugin_cmd(args(&["plugin", "enable", &id]), Some(id.clone()), cx));
        });
    }

    fn remove(&mut self, plugin: &Plugin, window: &mut Window, cx: &mut Context<Self>) {
        let id = plugin.id.clone();
        let this = cx.entity();
        let (title, message, button, command) = if plugin.source.installed() {
            (
                format!("Remove {}?", plugin.name()),
                "It is stopped and its installed copy deleted; its settings and data stay.".to_string(),
                "Remove",
                args(&["plugin", "remove", &id]),
            )
        } else {
            (
                format!("Unlink {}?", plugin.name()),
                "cornercase forgets it; its folder and its files stay where they are.".to_string(),
                "Unlink",
                args(&["plugin", "unlink", &id]),
            )
        };
        ask(title, message, button, true, window, cx, move |cx| {
            this.update(cx, |this, cx| this.plugin_cmd(command.clone(), Some(id.clone()), cx));
        });
    }

    fn link(&mut self, cx: &mut Context<Self>) {
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

    /// Installing shows what the plugin runs and asks before anything runs. A client cannot drive
    /// that preview yet (#468), so it happens in a terminal tab, where cornercase asks itself.
    fn install(&mut self, cx: &mut Context<Self>) {
        let source = self.source.read(cx).text().trim().to_string();
        if source.is_empty() {
            return;
        }
        self.source.update(cx, TextField::clear);
        in_a_tab(&["plugin", "install", &source], "install plugin", cx);
    }

    fn update(&mut self, id: Option<&str>, cx: &mut Context<Self>) {
        let mut words = vec!["plugin", "update"];
        words.extend(id);
        in_a_tab(&words, "update plugin", cx);
    }

    fn toggle_log(&mut self, id: String, cx: &mut Context<Self>) {
        if self.logs.remove(&id).is_none() {
            self.logs.insert(id.clone(), "…".into());
            self.read_log(id, cx);
        }
        cx.notify();
    }

    fn read_log(&mut self, id: String, cx: &mut Context<Self>) {
        store::run(cx, args(&["plugin", "logs", &id, "-n", LOG_LINES]), move |this, result, cx| {
            if let std::collections::hash_map::Entry::Occupied(mut e) = this.logs.entry(id) {
                let text = result.unwrap_or_else(|e| e.to_string());
                e.insert(if text.trim().is_empty() { "nothing yet".into() } else { text });
                cx.notify();
            }
        });
    }

    fn run_action(&mut self, action: &Action, cx: &mut Context<Self>) {
        let shown =
            Store::get(cx).read(cx).session.as_ref().map(super::super::model::Session::shown).unwrap_or_default();
        let Some(target) = target(&action.contexts, shown) else { return };
        let key = format!("{}/{}", action.plugin, action.action);
        let mut command = args(&["plugin", "run", &action.plugin, &action.action, "--json"]);
        command.extend(["--revision".to_string(), self.actions.revision.to_string()]);
        command.extend(target);
        self.busy.insert(key.clone());
        cx.notify();
        let title = action.title.clone();
        store::run(cx, command, move |this, result, cx| {
            this.busy.remove(&key);
            // A failed action exits with 1 but still prints its outcome; an error before it ran
            // (a stale revision, no server) prints nothing.
            let (text, kind) = match result.map(|out| serde_json::from_str::<Ran>(&out)) {
                Ok(Ok(ran)) => ran.say(&title),
                Ok(Err(e)) => (format!("{title}: {e}"), ToastKind::Error),
                Err(e) => (format!("{title}: {e}"), ToastKind::Error),
            };
            Store::get(cx).update(cx, |s, cx| s.toast(text, kind, cx));
            this.reload(cx);
        });
    }

    // -----------------------------------------------------------------------------------------
    // Drawing.

    fn render_plugin(&self, plugin: &Plugin, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let id = plugin.id.clone();
        let busy = self.busy.contains(&id);
        let manifest = plugin.manifest.as_ref();
        let (state, colour) = match (&plugin.error, plugin.enabled) {
            (Some(_), _) => ("cannot run", colours::failed()),
            (None, true) => ("on", colours::done()),
            (None, false) => ("off", colours::faint()),
        };
        let mut facts = vec![plugin.source.describe()];
        if let Some(m) = manifest {
            facts.push(m.asks());
            let runs = m.runs();
            if !runs.is_empty() {
                facts.push(format!("runs {}", runs.join(", ")));
            }
        }
        let actions: Vec<Action> = self.actions.actions.iter().filter(|a| a.plugin == id).cloned().collect();
        let log = self.logs.get(&id).cloned();
        let p = plugin.clone();
        let (p2, p3) = (plugin.clone(), plugin.clone());
        let (log_id, update_id) = (id.clone(), id.clone());
        let services = manifest.map_or(0, |m| m.service.len());
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .p(px(12.))
            .rounded(px(8.))
            .bg(colours::terminal())
            .border_1()
            .border_color(colours::border())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        ui::checkbox(SharedString::from(format!("switch-{id}")), plugin.enabled, "")
                            .when(plugin.error.is_none() && !busy, |d| {
                                d.on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.switch(&p, window, cx)))
                            })
                            .when(plugin.error.is_some() || busy, |d| d.opacity(0.4).cursor_default()),
                    )
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(plugin.name()))
                    .children(manifest.map(|m| ui::dim(m.version.clone())))
                    .child(ui::dim(id.clone()))
                    .child(div().flex_1())
                    .child(
                        div()
                            .px(px(6.))
                            .rounded(px(4.))
                            .bg(colours::sidebar())
                            .text_size(px(11.))
                            .text_color(colour)
                            .child(if busy { "…" } else { state }),
                    ),
            )
            .children(manifest.and_then(|m| m.description.clone()).filter(|d| !d.is_empty()).map(|d| {
                div().text_size(px(12.)).text_color(colours::muted()).child(d)
            }))
            .children(plugin.error.clone().map(|e| div().text_size(px(12.)).text_color(colours::failed()).child(e)))
            .child(ui::dim(facts.join(" · ")))
            .when(services > 0 && plugin.enabled, |d| {
                d.child(ui::dim("Whether its service runs needs usecornercase/cornercase-terminal#469"))
            })
            .when(!actions.is_empty(), |d| {
                d.child(
                    div().flex().flex_wrap().items_center().gap(px(6.)).pt(px(2.)).child(ui::dim("Actions")).children(
                        actions.into_iter().map(|action| {
                            let key = format!("{}/{}", action.plugin, action.action);
                            let running = self.busy.contains(&key);
                            let popup = action.placement == "popup";
                            let shown = Store::get(cx).read(cx).session.as_ref().map(super::super::model::Session::shown).unwrap_or_default();
                            let runnable = !popup && !running && target(&action.contexts, shown).is_some();
                            let label = if running { format!("{} …", action.title) } else { format!("▶ {}", action.title) };
                            ui::button(SharedString::from(format!("action-{key}")), label)
                                .when(!runnable, |b| b.opacity(0.45).cursor_default())
                                .when(runnable, |b| {
                                    b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.run_action(&action, cx)))
                                })
                                .into_any_element()
                        }),
                    ),
                )
            })
            .when(self.actions.actions.iter().any(|a| a.plugin == id && a.placement == "popup"), |d| {
                d.child(ui::dim("Popup actions open in cornercase's own window; showing them here needs usecornercase/cornercase-terminal#456"))
            })
            .child(
                div()
                    .flex()
                    .gap(px(6.))
                    .pt(px(4.))
                    .child(
                        ui::button(SharedString::from(format!("log-{id}")), if log.is_some() { "Hide log" } else { "Log" })
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_log(log_id.clone(), cx))),
                    )
                    .when(plugin.source.installed(), |d| {
                        d.child(
                            ui::button(SharedString::from(format!("update-{id}")), "Update…").on_click(cx.listener(
                                move |this, _: &ClickEvent, _, cx| this.update(Some(&update_id), cx),
                            )),
                        )
                    })
                    .child(div().flex_1())
                    .child(
                        ui::danger_button(
                            SharedString::from(format!("remove-{id}")),
                            if p2.source.installed() { "Remove" } else { "Unlink" },
                        )
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.remove(&p3, window, cx))),
                    ),
            )
            .children(log.map(|text| {
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
            }))
    }
}

impl Render for PluginsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.source.read(cx).focus_handle(cx).is_focused(window);
        let has_source = !self.source.read(cx).text().trim().is_empty();
        let installed = self.plugins.as_ref().is_some_and(|ps| ps.iter().any(|p| p.source.installed()));
        let list: Vec<_> = match (&self.plugins, &self.error) {
            (_, Some(e)) => vec![ui::note(e.clone()).text_color(colours::failed()).into_any_element()],
            (None, None) => vec![ui::note("Reading the plugins…").into_any_element()],
            (Some(ps), None) if ps.is_empty() => vec![
                ui::note("No plugins yet. Install one from GitHub or a git address above, or link a folder of yours while you write one.")
                    .into_any_element(),
            ],
            (Some(ps), None) => ps.clone().iter().map(|p| self.render_plugin(p, cx).into_any_element()).collect(),
        };
        div()
            .id("plugins")
            .size_full()
            .overflow_y_scroll()
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(colours::muted())
                    .child("A plugin runs as you and can do anything you can, including typing into every terminal. What it asks for is shown, not enforced. Plugins start off."),
            )
            .child(
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
                    .child(
                        ui::button("link", "Link folder…").on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.link(cx))),
                    )
                    .when(installed, |d| {
                        d.child(
                            ui::button("update-all", "Update all…")
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.update(None, cx))),
                        )
                    })
                    .child(
                        ui::icon_button("reload", "↻").py(px(3.)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.reload(cx))),
                    ),
            )
            .child(ui::dim(
                "Installing and updating show what the plugin runs and ask in a terminal tab; doing it here needs usecornercase/cornercase-terminal#468",
            ))
            .child(ui::section("Installed"))
            .children(list)
    }
}

pub(crate) fn build(window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|cx| PluginsPanel::new(window, cx)).into()
}

/// Runs `cornercase WORDS…` in a new tab of the shown workspace, where it can show what it does
/// and ask, and closes the sheet so the tab shows.
fn in_a_tab(words: &[&str], name: &str, cx: &mut App) {
    let line =
        std::iter::once("cornercase").chain(words.iter().copied()).map(shell_quote).collect::<Vec<_>>().join(" ");
    Store::get(cx).update(cx, |s, cx| {
        s.act(args(&["new-tab", "--focus", "--name", name, "--", &line]));
        s.close_sheet(cx);
    });
}

/// A word as a POSIX shell reads it back unchanged.
fn shell_quote(word: &str) -> String {
    if !word.is_empty() && word.chars().all(|c| c.is_ascii_alphanumeric() || "-_./:@=+,".contains(c)) {
        word.to_string()
    } else {
        format!("'{}'", word.replace('\'', r"'\''"))
    }
}

/// What an action runs on, as `plugin run` arguments: nothing for a button's (what the window
/// shows), else the most precise of its contexts the window shows. `None` when it shows none.
fn target(contexts: &[String], shown: Shown) -> Option<Vec<String>> {
    if contexts.iter().any(|c| c == "button") {
        return Some(Vec::new());
    }
    [("pane", shown.pane), ("tab", shown.tab), ("workspace", shown.workspace), ("project", shown.project)]
        .into_iter()
        .find_map(|(kind, id)| Some((kind, id?)).filter(|_| contexts.iter().any(|c| c == kind)))
        .map(|(kind, id)| vec![format!("--{kind}"), id.to_string()])
}

fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or_default().trim().to_string()
}

/// A question with a cancel and a confirm button, for this sheet's steps that want one.
struct Ask {
    title: String,
    message: String,
    button: String,
    danger: bool,
    on_yes: Box<dyn Fn(&mut App)>,
    focus: FocusHandle,
}

#[allow(clippy::too_many_arguments)]
fn ask(
    title: String,
    message: String,
    button: &str,
    danger: bool,
    window: &mut Window,
    cx: &mut App,
    on_yes: impl Fn(&mut App) + 'static,
) {
    let view = cx.new(|cx| Ask {
        title,
        message,
        button: button.into(),
        danger,
        on_yes: Box::new(on_yes),
        focus: cx.focus_handle(),
    });
    let focus = view.read(cx).focus.clone();
    Store::get(cx).update(cx, |s, cx| s.show_dialog(view.into(), cx));
    window.focus(&focus);
}

impl Ask {
    fn yes(&mut self, cx: &mut Context<Self>) {
        Store::get(cx).update(cx, Store::close_dialog);
        (self.on_yes)(cx);
    }
}

impl Render for Ask {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let close = |_: &ClickEvent, _: &mut Window, cx: &mut App| Store::get(cx).update(cx, Store::close_dialog);
        let button = if self.danger {
            ui::danger_button("yes", self.button.clone())
        } else {
            ui::primary_button("yes", self.button.clone())
        };
        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, e: &gpui::KeyDownEvent, _, cx| match e.keystroke.key.as_str() {
                "enter" => this.yes(cx),
                "escape" => Store::get(cx).update(cx, Store::close_dialog),
                _ => {}
            }))
            .child(dialogs::frame(
                self.title.clone(),
                div().text_color(colours::muted()).child(self.message.clone()),
                div()
                    .flex()
                    .gap(px(8.))
                    .child(ui::button("cancel", "Cancel").on_click(close))
                    .child(button.on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.yes(cx)))),
            ))
    }
}

// ---------------------------------------------------------------------------------------------
// What `cornercase plugin list --json`, `actions --json` and `run --json` print.

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct List {
    plugins: Vec<Plugin>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct Plugin {
    id: String,
    enabled: bool,
    source: Source,
    /// `None` when the manifest cannot be read; `error` says why.
    manifest: Option<Manifest>,
    error: Option<String>,
}

impl Plugin {
    fn name(&self) -> String {
        self.manifest.as_ref().map_or_else(|| self.id.clone(), |m| m.name.clone())
    }
}

/// Where a plugin comes from: `{"kind":"link","path"}` or `{"kind":"git","repo","url","ref",
/// "subdir","commit","path"}`, kept as JSON so a kind added later still reads.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(transparent)]
struct Source(Value);

impl Source {
    fn field(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(Value::as_str).filter(|s| !s.is_empty())
    }

    fn installed(&self) -> bool {
        self.field("kind") == Some("git")
    }

    fn describe(&self) -> String {
        match self.field("kind") {
            Some("link") => format!("linked from {}", self.field("path").unwrap_or("?")),
            Some("git") => {
                let place = self.field("repo").or(self.field("url")).unwrap_or("?");
                let subdir = self.field("subdir").map(|s| format!("/{s}")).unwrap_or_default();
                let at = match (self.field("ref"), self.field("commit")) {
                    (Some(r), Some(c)) => format!(" @{r} ({})", short(c)),
                    (None, Some(c)) => format!(" @{}", short(c)),
                    (Some(r), None) => format!(" @{r}"),
                    (None, None) => String::new(),
                };
                format!("installed from {place}{subdir}{at}")
            }
            Some(kind) => kind.to_string(),
            None => String::new(),
        }
    }
}

fn short(commit: &str) -> &str {
    commit.get(..12).unwrap_or(commit)
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct Manifest {
    name: String,
    version: String,
    description: Option<String>,
    permissions: Vec<String>,
    actions: Vec<Value>,
    events: Vec<Value>,
    service: Vec<Value>,
    window: Vec<Value>,
}

impl Manifest {
    /// What it asks for, as `plugin list` says it.
    fn asks(&self) -> String {
        if self.permissions.is_empty() {
            "asks for nothing".into()
        } else {
            format!("asks for {}", self.permissions.join(", "))
        }
    }

    /// What it runs: `2 actions`, `1 hook`, `1 service`, `1 window process`.
    fn runs(&self) -> Vec<String> {
        let count = |n: usize, one: &str, many: &str| match n {
            0 => None,
            1 => Some(format!("1 {one}")),
            n => Some(format!("{n} {many}")),
        };
        [
            count(self.actions.len(), "action", "actions"),
            count(self.events.len(), "hook", "hooks"),
            count(self.service.len(), "service", "services"),
            count(self.window.len(), "window process", "window processes"),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct Actions {
    actions: Vec<Action>,
    /// The plugins' revision these were listed at; `plugin run` refuses a stale one.
    revision: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct Action {
    plugin: String,
    action: String,
    title: String,
    contexts: Vec<String>,
    /// `job` or `popup`.
    placement: String,
}

/// The outcome of `plugin run --json`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct Ran {
    plugin: String,
    error: Option<String>,
    ms: u64,
}

impl Ran {
    fn say(&self, title: &str) -> (String, ToastKind) {
        match &self.error {
            None => (format!("{title}: done in {}", duration(self.ms)), ToastKind::Info),
            Some(e) => (format!("{title}: {e}; `cornercase plugin logs {}` says more", self.plugin), ToastKind::Error),
        }
    }
}

fn duration(ms: u64) -> String {
    if ms < 1000 { format!("{ms} ms") } else { format!("{:.1} s", ms as f64 / 1000.) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_plugin_list() {
        let text = r#"{"plugins": [
            {"enabled": true, "error": null, "id": "gpui.test", "log": "/x/plugin.log",
             "manifest": {"actions": [{"command": ["bin/hello"], "contexts": ["pane"], "id": "hello", "placement": "job", "title": "say hello"}],
                          "api": 1, "description": "A test", "events": [], "id": "gpui.test", "name": "GPUI test",
                          "permissions": ["read", "send"], "platforms": [], "service": [{"id": "s"}], "version": "0.1.0", "window": []},
             "source": {"kind": "link", "path": "/code/test"}},
            {"enabled": false, "error": "bin/x is missing", "id": "acme.notes", "manifest": null,
             "source": {"kind": "git", "repo": "acme/notes", "url": "https://github.com/acme/notes.git", "ref": "v1",
                        "subdir": "plugins/notes", "commit": "0123456789abcdef0123456789abcdef01234567", "path": "/p"}}
        ]}"#;
        let list: List = serde_json::from_str(text).unwrap();
        let (a, b) = (&list.plugins[0], &list.plugins[1]);
        assert_eq!(a.name(), "GPUI test");
        assert!(!a.source.installed());
        assert_eq!(a.source.describe(), "linked from /code/test");
        let m = a.manifest.as_ref().unwrap();
        assert_eq!(m.asks(), "asks for read, send");
        assert_eq!(m.runs(), ["1 action", "1 service"]);
        assert_eq!(b.name(), "acme.notes");
        assert!(b.source.installed());
        assert_eq!(b.source.describe(), "installed from acme/notes/plugins/notes @v1 (0123456789ab)");
        assert_eq!(b.error.as_deref(), Some("bin/x is missing"));
        assert_eq!(Manifest::default().asks(), "asks for nothing");
    }

    #[test]
    fn reads_actions_and_outcomes() {
        let actions: Actions = serde_json::from_str(
            r#"{"actions": [{"action": "hello", "contexts": ["workspace", "button"], "name": "GPUI test",
                "placement": "popup", "plugin": "gpui.test", "title": "say hello", "width": 80}], "revision": 2}"#,
        )
        .unwrap();
        assert_eq!(actions.revision, 2);
        assert_eq!(actions.actions[0].placement, "popup");
        let ok: Ran =
            serde_json::from_str(r#"{"plugin": "p", "error": null, "ms": 380, "ended": "exited", "code": 0}"#).unwrap();
        let (text, kind) = ok.say("say hello");
        assert_eq!(text, "say hello: done in 380 ms");
        assert!(kind == ToastKind::Info);
        let failed: Ran = serde_json::from_str(r#"{"plugin": "p", "error": "exit status: 3", "ms": 1500}"#).unwrap();
        assert_eq!(failed.say("x").0, "x: exit status: 3; `cornercase plugin logs p` says more");
        assert_eq!(duration(1500), "1.5 s");
    }

    #[test]
    fn runs_on_the_most_precise_place_shown() {
        let shown = Shown { project: Some(1), workspace: Some(2), tab: Some(3), pane: Some(4) };
        let ctx = |c: &[&str]| c.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(target(&ctx(&["workspace", "pane"]), shown), Some(vec!["--pane".into(), "4".into()]));
        assert_eq!(target(&ctx(&["project", "tab"]), shown), Some(vec!["--tab".into(), "3".into()]));
        assert_eq!(target(&ctx(&["pane", "button"]), shown), Some(vec![]));
        assert_eq!(target(&ctx(&["pane"]), Shown::default()), None);
        assert_eq!(target(&ctx(&[]), shown), None);
    }

    #[test]
    fn quotes_for_the_shell() {
        assert_eq!(shell_quote("acme/notes@v1.2"), "acme/notes@v1.2");
        assert_eq!(shell_quote("/My Plugins/x"), "'/My Plugins/x'");
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
        assert_eq!(shell_quote(""), "''");
    }
}
