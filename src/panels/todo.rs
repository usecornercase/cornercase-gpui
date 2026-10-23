//! The TODO list, docked beside the terminal: one global list that every client shares, live from
//! `status --app` (its `todos`), changed only through `cornercase todo` (add, done/undone, edit,
//! move, rm, clear-done, restore, send). Pending items come first, then done ones, as the server
//! orders them. Removing asks nothing: the panel offers an undo for a few seconds, as the TUI does.

use std::time::Duration;

use gpui::{
    AnyView, App, AppContext as _, ClickEvent, Context, Entity, Focusable as _, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, Window,
    div, prelude::FluentBuilder as _, px,
};
use serde::Deserialize;
use serde_json::Value;

use crate::model::Session;
use crate::store::{self, Store, ToastKind, args};
use crate::theme::colours;
use crate::ui::{self, TextField, TextFieldEvent};

/// How long the undo bar stays after a removal, as the TUI's undo toast.
const UNDO_FOR: Duration = Duration::from_secs(6);

/// An item of the list, as `status --app` and `todo list --json` print it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Item {
    pub id: u64,
    pub text: String,
    #[serde(default)]
    pub done: bool,
}

/// The items in `status --app`'s `todos`, in the server's order. Malformed items are skipped.
pub fn items(app: Option<&Value>) -> Vec<Item> {
    app.and_then(|app| app.get("todos"))
        .and_then(Value::as_array)
        .map(|todos| todos.iter().filter_map(|t| Item::deserialize(t).ok()).collect())
        .unwrap_or_default()
}

/// The `undo` number `todo rm --json` and `todo clear-done --json` answer, if anything was removed.
pub fn undo_number(answer: &Value) -> Option<u64> {
    answer.get("undo").and_then(Value::as_u64)
}

/// The `todo move` arguments that take an item one step up or down among its kind (pending or
/// done), or `None` when it is already at that end.
pub fn step(items: &[Item], id: u64, up: bool) -> Option<Vec<String>> {
    let item = items.iter().find(|i| i.id == id)?;
    let kind: Vec<&Item> = items.iter().filter(|i| i.done == item.done).collect();
    let at = kind.iter().position(|i| i.id == id)?;
    let id = id.to_string();
    if up {
        let before = kind.get(at.checked_sub(1)?)?;
        Some(args(&["todo", "move", &id, "--before", &before.id.to_string()]))
    } else if at + 1 >= kind.len() {
        None
    } else if let Some(before) = kind.get(at + 2) {
        Some(args(&["todo", "move", &id, "--before", &before.id.to_string()]))
    } else {
        Some(args(&["todo", "move", &id, "--last"]))
    }
}

/// The `todo move` arguments for dropping `dragged` on `target`: dropped on an item above it, it
/// goes before that item; on one below, after it, as lists usually reorder. The server keeps an
/// item among its kind. `None` when nothing would move.
pub fn drop_on(items: &[Item], dragged: u64, target: u64) -> Option<Vec<String>> {
    let from = items.iter().position(|i| i.id == dragged)?;
    let to = items.iter().position(|i| i.id == target)?;
    let id = dragged.to_string();
    if to == from {
        return None;
    }
    if to < from {
        return Some(args(&["todo", "move", &id, "--before", &target.to_string()]));
    }
    match items.get(to + 1) {
        Some(next) => Some(args(&["todo", "move", &id, "--before", &next.id.to_string()])),
        None => Some(args(&["todo", "move", &id, "--last"])),
    }
}

/// A pane running an agent that an item can be sent to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub pane: u64,
    pub status: Option<String>,
    pub label: String,
}

/// Every agent pane, in the sidebar's order, labelled as the TUI's send menu labels them:
/// `project › workspace · agent`, the workspace only when the project has several; labels that
/// would read the same get the tab's name, then the pane's id.
pub fn targets(session: &Session) -> Vec<Target> {
    let mut found: Vec<(Target, String)> = Vec::new();
    for project in &session.projects {
        let several = project.workspaces.len() > 1;
        for workspace in &project.workspaces {
            for tab in &workspace.tabs {
                for pane in tab.panes.iter().filter(|p| p.agent.is_some()) {
                    let agent = pane.agent.clone().unwrap_or_default();
                    let label = if several {
                        format!("{} › {} · {agent}", project.name, workspace.name)
                    } else {
                        format!("{} · {agent}", project.name)
                    };
                    let target = Target { pane: pane.id, status: pane.status.clone(), label };
                    found.push((target, tab.name.clone()));
                }
            }
        }
    }
    let same = |list: &[(Target, String)], label: &str| list.iter().filter(|(t, _)| t.label == label).count() > 1;
    let snapshot = found.clone();
    for (target, tab) in &mut found {
        if same(&snapshot, &target.label) && !tab.is_empty() {
            target.label = format!("{} ({tab})", target.label);
        }
    }
    let snapshot = found.clone();
    for (target, _) in &mut found {
        if same(&snapshot, &target.label) {
            target.label = format!("{} (pane {})", target.label, target.pane);
        }
    }
    found.into_iter().map(|(t, _)| t).collect()
}

/// What a confirmed `todo send --json` answer says, for the toast.
pub fn sent_message(answer: &Value, target: &str) -> (String, ToastKind) {
    if answer.get("unconfirmed").and_then(Value::as_bool) == Some(true) {
        return (format!("sent to {target}, not confirmed: stays to do"), ToastKind::Info);
    }
    if answer.get("checked").and_then(Value::as_bool) == Some(false) {
        return (format!("sent to {target}, but it changed meanwhile: stays to do"), ToastKind::Info);
    }
    (format!("sent to {target}"), ToastKind::Done)
}

/// The dragged item, and what follows the pointer while it moves.
#[derive(Clone)]
struct Dragged {
    id: u64,
    text: SharedString,
}

impl Render for Dragged {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .max_w(px(360.))
            .px(px(10.))
            .py(px(5.))
            .rounded(px(5.))
            .bg(colours::selected())
            .border_1()
            .border_color(colours::accent())
            .text_size(px(13.))
            .text_color(colours::text())
            .shadow_lg()
            .child(self.text.clone())
    }
}

/// The item being edited in place, with its field and the subscriptions that commit it.
struct Editing {
    id: u64,
    old: String,
    field: Entity<TextField>,
    _subscriptions: Vec<Subscription>,
}

/// What the last removal can be put back with.
struct Undo {
    number: u64,
    what: String,
}

pub struct TodoPanel {
    new_item: Entity<TextField>,
    editing: Option<Editing>,
    /// The item whose agent list is open.
    sending: Option<u64>,
    undo: Option<Undo>,
    _subscriptions: Vec<Subscription>,
}

impl TodoPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let new_item = cx.new(|cx| TextField::new("New TODO", cx));
        let subscriptions = vec![
            cx.observe(&Store::get(cx), |_, _, cx| cx.notify()),
            cx.subscribe_in(&new_item, window, |this, field, event, window, cx| match event {
                TextFieldEvent::Submit => this.add(window, cx),
                TextFieldEvent::Cancel => {
                    field.update(cx, |f, cx| f.clear(cx));
                    window.blur();
                }
                TextFieldEvent::Changed => cx.notify(),
            }),
        ];
        Self { new_item, editing: None, sending: None, undo: None, _subscriptions: subscriptions }
    }

    fn cmd(&self, command: Vec<String>, cx: &mut Context<Self>) {
        Store::get(cx).read(cx).cmd(command);
    }

    /// `todo add` with the field's text; the field stays open for the next item.
    fn add(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let text = self.new_item.read(cx).text().trim().to_string();
        if text.is_empty() {
            return;
        }
        self.new_item.update(cx, |f, cx| f.clear(cx));
        // `--` so an item starting with a dash is text, not an option.
        self.cmd(args(&["todo", "add", "--", &text]), cx);
    }

    fn toggle(&mut self, item: &Item, cx: &mut Context<Self>) {
        let id = item.id.to_string();
        // `undone` is idempotent; `done` is a toggle, sent only for a pending item.
        let verb = if item.done { "undone" } else { "done" };
        self.cmd(args(&["todo", verb, &id]), cx);
    }

    fn start_edit(&mut self, item: &Item, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_edit(cx);
        let field = cx.new(|cx| {
            let mut field = TextField::new("What to do", cx);
            field.set_text(item.text.clone(), cx);
            field
        });
        let focus = field.read(cx).focus_handle(cx);
        let subscriptions = vec![
            cx.subscribe_in(&field, window, |this, _, event, window, cx| match event {
                TextFieldEvent::Submit => {
                    this.commit_edit(cx);
                    window.blur();
                }
                TextFieldEvent::Cancel => {
                    this.editing = None;
                    cx.notify();
                }
                TextFieldEvent::Changed => cx.notify(),
            }),
            // Clicking anywhere else saves, so nothing typed is lost.
            cx.on_blur(&focus, window, |this, _, cx| this.commit_edit(cx)),
        ];
        self.editing = Some(Editing { id: item.id, old: item.text.clone(), field, _subscriptions: subscriptions });
        self.sending = None;
        window.focus(&focus);
        cx.notify();
    }

    /// Saves the item being edited, if its text changed; an empty text keeps the old one.
    fn commit_edit(&mut self, cx: &mut Context<Self>) {
        let Some(editing) = self.editing.take() else { return };
        let text = editing.field.read(cx).text().trim().to_string();
        if !text.is_empty() && text != editing.old {
            self.cmd(args(&["todo", "edit", &editing.id.to_string(), "--", &text]), cx);
        }
        cx.notify();
    }

    /// Runs a removal (`rm` or `clear-done`) and keeps its undo number for the bar.
    fn remove(&mut self, command: Vec<String>, what: String, cx: &mut Context<Self>) {
        store::run_json(cx, command, move |this: &mut Self, answer, cx| match answer {
            Ok(answer) => {
                if let Some(number) = undo_number(&answer) {
                    this.undo = Some(Undo { number, what });
                    cx.notify();
                    cx.spawn(async move |this, cx| {
                        cx.background_executor().timer(UNDO_FOR).await;
                        let _ = this.update(cx, |this, cx| {
                            if this.undo.as_ref().is_some_and(|u| u.number == number) {
                                this.undo = None;
                                cx.notify();
                            }
                        });
                    })
                    .detach();
                }
            }
            Err(e) => Store::get(cx).update(cx, |s, cx| s.toast(e.to_string(), ToastKind::Error, cx)),
        });
    }

    fn restore(&mut self, cx: &mut Context<Self>) {
        if let Some(undo) = self.undo.take() {
            self.cmd(args(&["todo", "restore", &undo.number.to_string()]), cx);
            cx.notify();
        }
    }

    /// `todo send ID --pane P`: types the item into the agent; the server checks it off once the
    /// agent took it, and refuses (with its reason) when the agent cannot take a prompt now.
    fn send(&mut self, id: u64, target: Target, cx: &mut Context<Self>) {
        self.sending = None;
        let command = args(&["todo", "send", &id.to_string(), "--pane", &target.pane.to_string(), "--json"]);
        let label = target.label;
        store::run_json(cx, command, move |_: &mut Self, answer, cx| {
            let (text, kind) = match answer {
                Ok(answer) => sent_message(&answer, &label),
                Err(e) => (e.to_string(), ToastKind::Error),
            };
            Store::get(cx).update(cx, |s, cx| s.toast(text, kind, cx));
        });
        cx.notify();
    }

    // -----------------------------------------------------------------------------------------
    // Drawing.

    fn render_item(&self, item: &Item, items: &[Item], window: &mut Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        let id = item.id;
        let group = SharedString::from(format!("todo-{id}"));
        let editing = self.editing.as_ref().filter(|e| e.id == id);
        let first = items.iter().filter(|i| i.done == item.done).map(|i| i.id).next() == Some(id);
        let last = items.iter().filter(|i| i.done == item.done).map(|i| i.id).last() == Some(id);

        let check = {
            let item = item.clone();
            div()
                .id(("todo-check", id))
                .flex_none()
                .w(px(18.))
                .pt(px(1.))
                .text_center()
                .cursor_pointer()
                .text_color(if item.done { colours::done() } else { colours::muted() })
                .hover(|s| s.text_color(colours::accent()))
                .child(if item.done { "✓" } else { "○" })
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle(&item, cx)))
        };

        let text: gpui::AnyElement = match editing {
            Some(editing) => {
                let focused = editing.field.read(cx).focus_handle(cx).is_focused(window);
                div().flex_1().min_w(px(0.)).child(ui::input_box(editing.field.clone(), focused)).into_any_element()
            }
            None => {
                let item_for_click = item.clone();
                div()
                    .id(("todo-text", id))
                    .flex_1()
                    .min_w(px(0.))
                    .cursor_text()
                    .when(item.done, |d| d.text_color(colours::faint()).line_through())
                    .child(item.text.clone())
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.start_edit(&item_for_click, window, cx)
                    }))
                    .into_any_element()
            }
        };

        // The buttons show on hover; their column is always there so the text never reflows.
        let mut buttons = div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(1.))
            .opacity(if editing.is_some() || self.sending == Some(id) { 1. } else { 0. })
            .group_hover(group.clone(), |s| s.opacity(1.));
        if !item.done && editing.is_none() {
            buttons = buttons.child(
                ui::icon_button(("todo-send", id), "▸")
                    .text_size(px(15.))
                    .text_color(if self.sending == Some(id) { colours::accent() } else { colours::faint() })
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.sending = if this.sending == Some(id) { None } else { Some(id) };
                        cx.notify();
                    })),
            );
        }
        let up = step(items, id, true);
        let down = step(items, id, false);
        buttons = buttons
            .child(arrow(("todo-up", id), "↑", up, first, cx))
            .child(arrow(("todo-down", id), "↓", down, last, cx));
        let what = item.text.clone();
        buttons = buttons.child(ui::icon_button(("todo-rm", id), "✕").on_click(cx.listener(
            move |this, _: &ClickEvent, _, cx| {
                if this.editing.as_ref().is_some_and(|e| e.id == id) {
                    this.editing = None;
                }
                let command = args(&["todo", "rm", &id.to_string(), "--json"]);
                this.remove(command, format!("Removed “{}”", short(&what, 40)), cx);
            },
        )));

        let dragged = Dragged { id, text: item.text.clone().into() };
        let row = div()
            .id(("todo", id))
            .group(group)
            .flex()
            .items_start()
            .gap(px(6.))
            .px(px(6.))
            .py(px(5.))
            .rounded(px(5.))
            .text_size(px(13.))
            .text_color(colours::text())
            .hover(|s| s.bg(colours::hover()))
            .when(self.sending == Some(id), |d| d.bg(colours::selected()))
            .drag_over::<Dragged>(|s, _, _, _| s.border_t_2().border_color(colours::accent()))
            .on_drop(cx.listener(move |this, dragged: &Dragged, _, cx| {
                let items = items_of(cx);
                if let Some(command) = drop_on(&items, dragged.id, id) {
                    this.cmd(command, cx);
                }
            }))
            .child(check)
            .child(text)
            .child(buttons);
        let row = if editing.is_none() {
            row.on_drag(dragged, |dragged, _, _, cx| cx.new(|_| dragged.clone())).into_any_element()
        } else {
            row.into_any_element()
        };

        if self.sending != Some(id) {
            return row;
        }
        div().flex().flex_col().child(row).child(self.render_targets(id, cx)).into_any_element()
    }

    /// The agents an item can go to, under the item, as the TUI's menu at the pointer.
    fn render_targets(&self, id: u64, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let store = Store::get(cx);
        let targets = store.read(cx).session.as_ref().map(targets).unwrap_or_default();
        let mut list = div()
            .ml(px(24.))
            .mr(px(6.))
            .mb(px(6.))
            .p(px(4.))
            .flex()
            .flex_col()
            .rounded(px(6.))
            .bg(colours::panel())
            .border_1()
            .border_color(colours::border())
            .child(div().px(px(6.)).py(px(2.)).child(ui::dim("SEND TO")));
        if targets.is_empty() {
            return list.child(ui::note("no agent is running"));
        }
        for target in targets {
            let pane = target.pane;
            let label = target.label.clone();
            list = list.child(
                ui::row(("todo-target", pane), false)
                    .text_size(px(12.))
                    .child(ui::badge(target.status.as_deref()))
                    .child(div().flex_1().min_w(px(0.)).truncate().child(label))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.send(id, target.clone(), cx))),
            );
        }
        list
    }
}

/// An up or down button; dimmed when the item is at that end of its kind.
fn arrow(
    id: (&'static str, u64),
    glyph: &'static str,
    command: Option<Vec<String>>,
    at_end: bool,
    cx: &mut Context<TodoPanel>,
) -> impl IntoElement + use<> {
    let button = ui::icon_button(id, glyph);
    match command {
        Some(command) if !at_end => button.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.cmd(command.clone(), cx);
        })),
        _ => button.opacity(0.3).cursor_default(),
    }
}

/// The list as the store has it now.
fn items_of(cx: &App) -> Vec<Item> {
    items(Store::get(cx).read(cx).app.as_ref())
}

/// The first `max` characters of a text, with an ellipsis when cut.
fn short(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let cut: String = text.chars().take(max.saturating_sub(1)).collect();
    format!("{}…", cut.trim_end())
}

impl Render for TodoPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let store = Store::get(cx);
        let loaded = store.read(cx).app.is_some();
        let items = items_of(cx);
        // An item removed elsewhere closes what was open on it.
        if self.editing.as_ref().is_some_and(|e| !items.iter().any(|i| i.id == e.id)) {
            self.editing = None;
        }
        if self.sending.is_some_and(|id| !items.iter().any(|i| i.id == id && !i.done)) {
            self.sending = None;
        }
        let pending = items.iter().filter(|i| !i.done).count();
        let done = items.len() - pending;

        let header = div()
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .px(px(12.))
            .py(px(6.))
            .child(ui::dim(match (pending, done) {
                (0, 0) => "nothing to do".to_string(),
                (p, 0) => format!("{p} to do"),
                (p, d) => format!("{p} to do · {d} done"),
            }))
            .when(done > 0, |d| {
                d.child(ui::button("todo-clear-done", "Clear done").on_click(cx.listener(
                    move |this, _: &ClickEvent, _, cx| {
                        let what = if done == 1 { "Cleared 1 done item".into() } else { format!("Cleared {done} done items") };
                        this.remove(args(&["todo", "clear-done", "--json"]), what, cx);
                    },
                )))
            });

        let mut list = div().flex().flex_col().gap(px(1.)).px(px(6.));
        if !loaded {
            list = list.child(ui::note("Loading…"));
        } else if items.is_empty() {
            list = list.child(ui::note("Nothing to do. Add an item below; any client sees the same list."));
        }
        let mut seen_done = false;
        for item in &items {
            if item.done && !seen_done {
                seen_done = true;
                list = list.child(div().px(px(6.)).child(ui::section("Done")));
            }
            list = list.child(self.render_item(item, &items, window, cx));
        }
        // Dropping below the last item puts it at the end of its kind.
        list = list.child(
            div()
                .id("todo-end")
                .h(px(24.))
                .rounded(px(5.))
                .drag_over::<Dragged>(|s, _, _, _| s.border_t_2().border_color(colours::accent()))
                .on_drop(cx.listener(|this, dragged: &Dragged, _, cx| {
                    let items = items_of(cx);
                    if items.last().is_some_and(|last| last.id != dragged.id) {
                        this.cmd(args(&["todo", "move", &dragged.id.to_string(), "--last"]), cx);
                    }
                })),
        );

        let undo = self.undo.as_ref().map(|undo| {
            div()
                .flex_none()
                .mx(px(10.))
                .mb(px(6.))
                .px(px(10.))
                .py(px(6.))
                .flex()
                .items_center()
                .gap(px(8.))
                .rounded(px(6.))
                .bg(colours::panel())
                .border_1()
                .border_color(colours::border())
                .child(div().flex_1().min_w(px(0.)).truncate().text_size(px(12.)).child(undo.what.clone()))
                .child(
                    ui::primary_button("todo-undo", "Undo")
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.restore(cx))),
                )
        });

        let focused = self.new_item.read(cx).focus_handle(cx).is_focused(window);
        let new_item = div().flex_none().px(px(10.)).pb(px(10.)).pt(px(4.)).child(ui::input_box(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(div().text_color(if focused { colours::accent() } else { colours::faint() }).child("+"))
                .child(div().flex_1().child(self.new_item.clone())),
            focused,
        ));

        div()
            .size_full()
            .flex()
            .flex_col()
            .text_color(colours::text())
            .child(header)
            .child(div().id("todo-list").flex_1().overflow_y_scroll().child(list))
            .children(undo)
            .child(new_item)
    }
}

pub fn build(window: &mut Window, cx: &mut gpui::App) -> AnyView {
    cx.new(|cx| TodoPanel::new(window, cx)).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{PaneInfo, Project, Tab, Workspace};
    use serde_json::json;

    fn item(id: u64, done: bool) -> Item {
        Item { id, text: format!("item {id}"), done }
    }

    fn moved(command: Option<Vec<String>>) -> Option<String> {
        command.map(|c| c[2..].join(" "))
    }

    #[test]
    fn reads_the_app_feed() {
        let app = json!({ "todos": [
            { "id": 3, "text": "review the login fix", "done": false },
            { "id": 1, "text": "ship", "done": true },
            { "text": "no id" },
        ], "queue": [] });
        assert_eq!(
            items(Some(&app)),
            vec![Item { id: 3, text: "review the login fix".into(), done: false }, Item {
                id: 1,
                text: "ship".into(),
                done: true
            }]
        );
        assert!(items(Some(&json!({}))).is_empty());
        assert!(items(None).is_empty());
    }

    #[test]
    fn reads_the_undo_number() {
        assert_eq!(undo_number(&json!({ "todos": [{ "id": 1, "text": "x", "done": true }], "undo": 7 })), Some(7));
        assert_eq!(undo_number(&json!({ "todos": [] })), None);
    }

    #[test]
    fn steps_within_its_kind() {
        let list = [item(1, false), item(2, false), item(3, false), item(4, true), item(5, true)];
        assert_eq!(moved(step(&list, 1, true)), None);
        assert_eq!(moved(step(&list, 2, true)).as_deref(), Some("2 --before 1"));
        assert_eq!(moved(step(&list, 1, false)).as_deref(), Some("1 --before 3"));
        assert_eq!(moved(step(&list, 2, false)).as_deref(), Some("2 --last"));
        assert_eq!(moved(step(&list, 3, false)), None);
        assert_eq!(moved(step(&list, 4, true)), None);
        assert_eq!(moved(step(&list, 4, false)).as_deref(), Some("4 --last"));
        assert_eq!(moved(step(&list, 5, true)).as_deref(), Some("5 --before 4"));
        assert_eq!(moved(step(&list, 9, true)), None);
    }

    #[test]
    fn drops_before_above_and_after_below() {
        let list = [item(1, false), item(2, false), item(3, false), item(4, true)];
        assert_eq!(moved(drop_on(&list, 3, 1)).as_deref(), Some("3 --before 1"));
        assert_eq!(moved(drop_on(&list, 1, 2)).as_deref(), Some("1 --before 3"));
        assert_eq!(moved(drop_on(&list, 1, 4)).as_deref(), Some("1 --last"));
        assert_eq!(moved(drop_on(&list, 2, 2)), None);
    }

    fn pane(id: u64, agent: Option<&str>) -> PaneInfo {
        PaneInfo { id, agent: agent.map(Into::into), status: Some("idle".into()), ..Default::default() }
    }

    #[test]
    fn labels_agents_like_the_tui() {
        let session = Session {
            projects: vec![
                Project {
                    name: "web-shop".into(),
                    workspaces: vec![
                        Workspace {
                            name: "default".into(),
                            tabs: vec![Tab {
                                name: "claude".into(),
                                panes: vec![pane(1, Some("claude")), pane(2, None)],
                                ..Default::default()
                            }],
                            ..Default::default()
                        },
                        Workspace {
                            name: "feat/x".into(),
                            tabs: vec![
                                Tab { name: "a".into(), panes: vec![pane(3, Some("codex"))], ..Default::default() },
                                Tab { name: "b".into(), panes: vec![pane(4, Some("codex"))], ..Default::default() },
                                Tab { name: "b".into(), panes: vec![pane(5, Some("codex"))], ..Default::default() },
                            ],
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                },
                Project {
                    name: "api".into(),
                    workspaces: vec![Workspace {
                        tabs: vec![Tab { panes: vec![pane(6, Some("claude"))], ..Default::default() }],
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let labels: Vec<(u64, String)> = targets(&session).into_iter().map(|t| (t.pane, t.label)).collect();
        assert_eq!(labels, vec![
            (1, "web-shop › default · claude".to_string()),
            (3, "web-shop › feat/x · codex (a)".to_string()),
            (4, "web-shop › feat/x · codex (b) (pane 4)".to_string()),
            (5, "web-shop › feat/x · codex (b) (pane 5)".to_string()),
            (6, "api · claude".to_string()),
        ]);
    }

    #[test]
    fn says_how_a_send_went() {
        assert!(matches!(sent_message(&json!({ "pane": 5 }), "x").1, ToastKind::Done));
        assert_eq!(sent_message(&json!({ "unconfirmed": true }), "x").0, "sent to x, not confirmed: stays to do");
        assert!(matches!(sent_message(&json!({ "checked": false }), "x").1, ToastKind::Info));
    }

    #[test]
    fn shortens_long_texts() {
        assert_eq!(short("short", 10), "short");
        assert_eq!(short("a rather long item text", 10), "a rather…");
    }
}
