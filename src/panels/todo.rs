mod data;

use std::time::Duration;

use gpui::{
    AnyElement, AnyView, App, AppContext as _, ClickEvent, Context, Div, Entity, Focusable as _,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Subscription, Window, div, prelude::FluentBuilder as _, px,
};

use crate::panels::queue::toast_outcome;
use crate::store::{self, Store, ToastKind, args};
use crate::theme::colours;
use crate::ui::{self, TextField, TextFieldEvent};
use data::{Item, Target, drop_on, first_and_last, items, sent_message, short, step, targets, undo_number};

const UNDO_FOR: Duration = Duration::from_secs(6);

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

struct Editing {
    id: u64,
    old: String,
    field: Entity<TextField>,
    _subscriptions: Vec<Subscription>,
}

struct Undo {
    number: u64,
    what: String,
}

pub(crate) struct TodoPanel {
    new_item: Entity<TextField>,
    editing: Option<Editing>,
    sending: Option<u64>,
    undo: Option<Undo>,
    _subscriptions: Vec<Subscription>,
}

fn cmd(command: Vec<String>, cx: &App) {
    Store::get(cx).read(cx).cmd(command);
}

fn items_of(cx: &App) -> Vec<Item> {
    items(Store::get(cx).read(cx).app.as_ref())
}

fn counts_label(pending: usize, done: usize) -> String {
    match (pending, done) {
        (0, 0) => "nothing to do".to_string(),
        (p, 0) => format!("{p} to do"),
        (p, d) => format!("{p} to do · {d} done"),
    }
}

fn cleared_label(done: usize) -> String {
    if done == 1 { "Cleared 1 done item".into() } else { format!("Cleared {done} done items") }
}

fn drop_marker<E: gpui::InteractiveElement>(element: E) -> E {
    element.drag_over::<Dragged>(|s, _, _, _| s.border_t_2().border_color(colours::accent()))
}

impl TodoPanel {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let new_item = cx.new(|cx| TextField::new("New TODO", cx));
        let subscriptions = vec![
            cx.observe(&Store::get(cx), |_, _, cx| cx.notify()),
            cx.subscribe_in(&new_item, window, |this, field, event, window, cx| match event {
                TextFieldEvent::Submit => this.add(cx),
                TextFieldEvent::Cancel => {
                    field.update(cx, TextField::clear);
                    window.blur();
                }
                TextFieldEvent::Changed => cx.notify(),
            }),
        ];
        Self { new_item, editing: None, sending: None, undo: None, _subscriptions: subscriptions }
    }

    fn add(&mut self, cx: &mut Context<Self>) {
        let text = self.new_item.read(cx).text().trim().to_string();
        if text.is_empty() {
            return;
        }
        self.new_item.update(cx, TextField::clear);
        cmd(args(&["todo", "add", "--", &text]), cx);
    }

    fn toggle(item: &Item, cx: &mut Context<Self>) {
        let verb = if item.done { "undone" } else { "done" };
        cmd(args(&["todo", verb, &item.id.to_string()]), cx);
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
                TextFieldEvent::Cancel => this.cancel_edit(cx),
                TextFieldEvent::Changed => cx.notify(),
            }),
            cx.on_blur(&focus, window, |this, _, cx| this.commit_edit(cx)),
        ];
        self.editing = Some(Editing { id: item.id, old: item.text.clone(), field, _subscriptions: subscriptions });
        self.sending = None;
        window.focus(&focus);
        cx.notify();
    }

    fn cancel_edit(&mut self, cx: &mut Context<Self>) {
        self.editing = None;
        cx.notify();
    }

    fn commit_edit(&mut self, cx: &mut Context<Self>) {
        let Some(editing) = self.editing.take() else { return };
        let text = editing.field.read(cx).text().trim().to_string();
        if !text.is_empty() && text != editing.old {
            cmd(args(&["todo", "edit", &editing.id.to_string(), "--", &text]), cx);
        }
        cx.notify();
    }

    fn remove(command: Vec<String>, what: String, cx: &mut Context<Self>) {
        store::run_json(cx, command, move |this: &mut Self, answer, cx| match answer {
            Ok(answer) => {
                if let Some(number) = undo_number(&answer) {
                    this.offer_undo(number, what, cx);
                }
            }
            Err(e) => Store::get(cx).update(cx, |s, cx| s.toast(e.to_string(), ToastKind::Error, cx)),
        });
    }

    fn offer_undo(&mut self, number: u64, what: String, cx: &mut Context<Self>) {
        self.undo = Some(Undo { number, what });
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(UNDO_FOR).await;
            let _ = this.update(cx, |this, cx| this.expire_undo(number, cx));
        })
        .detach();
    }

    fn expire_undo(&mut self, number: u64, cx: &mut Context<Self>) {
        if self.undo.as_ref().is_some_and(|u| u.number == number) {
            self.undo = None;
            cx.notify();
        }
    }

    fn remove_item(&mut self, id: u64, what: &str, cx: &mut Context<Self>) {
        if self.editing.as_ref().is_some_and(|e| e.id == id) {
            self.editing = None;
        }
        let command = args(&["todo", "rm", &id.to_string(), "--json"]);
        Self::remove(command, format!("Removed “{}”", short(what, 40)), cx);
    }

    fn restore(&mut self, cx: &mut Context<Self>) {
        if let Some(undo) = self.undo.take() {
            cmd(args(&["todo", "restore", &undo.number.to_string()]), cx);
            cx.notify();
        }
    }

    fn toggle_sending(&mut self, id: u64, cx: &mut Context<Self>) {
        self.sending = if self.sending == Some(id) { None } else { Some(id) };
        cx.notify();
    }

    fn send(&mut self, id: u64, target: Target, cx: &mut Context<Self>) {
        self.sending = None;
        let command = args(&["todo", "send", &id.to_string(), "--pane", &target.pane.to_string(), "--json"]);
        let label = target.label;
        store::run_json(cx, command, move |_: &mut Self, answer, cx| {
            toast_outcome(answer, |answer| sent_message(&answer, &label), cx);
        });
        cx.notify();
    }

    fn drop_item(dragged: u64, target: u64, cx: &mut Context<Self>) {
        if let Some(command) = drop_on(&items_of(cx), dragged, target) {
            cmd(command, cx);
        }
    }

    fn drop_at_end(dragged: u64, cx: &mut Context<Self>) {
        if items_of(cx).last().is_some_and(|last| last.id != dragged) {
            cmd(args(&["todo", "move", &dragged.to_string(), "--last"]), cx);
        }
    }

    fn forget_missing(&mut self, items: &[Item]) {
        if self.editing.as_ref().is_some_and(|e| !items.iter().any(|i| i.id == e.id)) {
            self.editing = None;
        }
        if self.sending.is_some_and(|id| !items.iter().any(|i| i.id == id && !i.done)) {
            self.sending = None;
        }
    }
}

fn arrow(
    id: (&'static str, u64),
    glyph: &'static str,
    command: Option<Vec<String>>,
    at_end: bool,
    cx: &mut Context<TodoPanel>,
) -> impl IntoElement + use<> {
    let button = ui::icon_button(id, glyph);
    match command {
        Some(command) if !at_end => {
            button.on_click(cx.listener(move |_, _: &ClickEvent, _, cx| cmd(command.clone(), cx)))
        }
        _ => button.opacity(0.3).cursor_default(),
    }
}

fn check(item: &Item, cx: &mut Context<TodoPanel>) -> impl IntoElement + use<> {
    let toggled = item.clone();
    div()
        .id(("todo-check", item.id))
        .flex_none()
        .w(px(18.))
        .pt(px(1.))
        .text_center()
        .cursor_pointer()
        .text_color(if item.done { colours::done() } else { colours::muted() })
        .hover(|s| s.text_color(colours::accent()))
        .child(if item.done { "✓" } else { "○" })
        .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| TodoPanel::toggle(&toggled, cx)))
}

impl TodoPanel {
    fn header(items: &[Item], cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let pending = items.iter().filter(|i| !i.done).count();
        let done = items.len() - pending;
        div()
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .px(px(12.))
            .py(px(6.))
            .child(ui::dim(counts_label(pending, done)))
            .when(done > 0, |d| {
                d.child(ui::button("todo-clear-done", "Clear done").on_click(cx.listener(
                    move |_, _: &ClickEvent, _, cx| {
                        Self::remove(args(&["todo", "clear-done", "--json"]), cleared_label(done), cx);
                    },
                )))
            })
    }

    fn list(&self, loaded: bool, items: &[Item], window: &Window, cx: &mut Context<Self>) -> Div {
        let mut list = div().flex().flex_col().gap(px(1.)).px(px(6.));
        if !loaded {
            list = list.child(ui::note("Loading…"));
        } else if items.is_empty() {
            list = list.child(ui::note("Nothing to do. Add an item below; any client sees the same list."));
        }
        let first_done = items.iter().position(|i| i.done);
        for (at, item) in items.iter().enumerate() {
            if Some(at) == first_done {
                list = list.child(div().px(px(6.)).child(ui::section("Done")));
            }
            list = list.child(self.item_row(item, items, window, cx));
        }
        list.child(Self::end_drop(cx))
    }

    fn end_drop(cx: &mut Context<Self>) -> impl IntoElement + use<> {
        drop_marker(div().id("todo-end").h(px(24.)).rounded(px(5.)))
            .on_drop(cx.listener(|_, dragged: &Dragged, _, cx| Self::drop_at_end(dragged.id, cx)))
    }

    fn item_row(&self, item: &Item, items: &[Item], window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let id = item.id;
        let editing = self.editing.as_ref().is_some_and(|e| e.id == id);
        let sending = self.sending == Some(id);
        let target = id;
        let row = drop_marker(
            div()
                .id(("todo", id))
                .group(SharedString::from(format!("todo-{id}")))
                .flex()
                .items_start()
                .gap(px(6.))
                .px(px(6.))
                .py(px(5.))
                .rounded(px(5.))
                .text_size(px(13.))
                .text_color(colours::text())
                .hover(|s| s.bg(colours::hover()))
                .when(sending, |d| d.bg(colours::selected())),
        )
        .on_drop(cx.listener(move |_, dragged: &Dragged, _, cx| Self::drop_item(dragged.id, target, cx)))
        .child(check(item, cx))
        .child(self.item_text(item, window, cx))
        .child(self.item_buttons(item, items, cx));
        let dragged = Dragged { id, text: item.text.clone().into() };
        let row = if editing {
            row.into_any_element()
        } else {
            row.on_drag(dragged, |dragged, _, _, cx| cx.new(|_| dragged.clone())).into_any_element()
        };
        if !sending {
            return row;
        }
        div().flex().flex_col().child(row).child(Self::targets_menu(id, cx)).into_any_element()
    }

    fn item_text(&self, item: &Item, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        if let Some(editing) = self.editing.as_ref().filter(|e| e.id == item.id) {
            let focused = editing.field.read(cx).focus_handle(cx).is_focused(window);
            return div()
                .flex_1()
                .min_w(px(0.))
                .child(ui::input_box(editing.field.clone(), focused))
                .into_any_element();
        }
        let clicked = item.clone();
        div()
            .id(("todo-text", item.id))
            .flex_1()
            .min_w(px(0.))
            .cursor_text()
            .when(item.done, |d| d.text_color(colours::faint()).line_through())
            .child(item.text.clone())
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.start_edit(&clicked, window, cx)))
            .into_any_element()
    }

    fn item_buttons(&self, item: &Item, items: &[Item], cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let id = item.id;
        let editing = self.editing.as_ref().is_some_and(|e| e.id == id);
        let sending = self.sending == Some(id);
        let (first, last) = first_and_last(items, item);
        let what = item.text.clone();
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(1.))
            .opacity(if editing || sending { 1. } else { 0. })
            .group_hover(SharedString::from(format!("todo-{id}")), |s| s.opacity(1.))
            .when(!item.done && !editing, |d| {
                d.child(
                    ui::icon_button(("todo-send", id), "▸")
                        .text_size(px(15.))
                        .text_color(if sending { colours::accent() } else { colours::faint() })
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_sending(id, cx))),
                )
            })
            .child(arrow(("todo-up", id), "↑", step(items, id, true), first, cx))
            .child(arrow(("todo-down", id), "↓", step(items, id, false), last, cx))
            .child(
                ui::icon_button(("todo-rm", id), "✕")
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.remove_item(id, &what, cx))),
            )
    }

    fn targets_menu(id: u64, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let targets = Store::get(cx).read(cx).session.as_ref().map(targets).unwrap_or_default();
        let list = div()
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
        let rows: Vec<AnyElement> = targets.into_iter().map(|target| Self::target_row(id, target, cx)).collect();
        list.children(rows)
    }

    fn target_row(id: u64, target: Target, cx: &mut Context<Self>) -> AnyElement {
        ui::row(("todo-target", target.pane), false)
            .text_size(px(12.))
            .child(ui::badge(target.status.as_deref()))
            .child(div().flex_1().min_w(px(0.)).truncate().child(target.label.clone()))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.send(id, target.clone(), cx)))
            .into_any_element()
    }

    fn undo_bar(&self, cx: &mut Context<Self>) -> Option<Div> {
        let undo = self.undo.as_ref()?;
        Some(
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
                ),
        )
    }

    fn new_item_field(&self, window: &Window, cx: &App) -> impl IntoElement + use<> {
        let focused = self.new_item.read(cx).focus_handle(cx).is_focused(window);
        div().flex_none().px(px(10.)).pb(px(10.)).pt(px(4.)).child(ui::input_box(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(div().text_color(if focused { colours::accent() } else { colours::faint() }).child("+"))
                .child(div().flex_1().child(self.new_item.clone())),
            focused,
        ))
    }
}

impl Render for TodoPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let loaded = Store::get(cx).read(cx).app.is_some();
        let items = items_of(cx);
        self.forget_missing(&items);
        let header = Self::header(&items, cx);
        let list = self.list(loaded, &items, window, cx);
        let undo = self.undo_bar(cx);
        let new_item = self.new_item_field(window, cx);
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

pub(crate) fn build(window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|cx| TodoPanel::new(window, cx)).into()
}

#[cfg(test)]
mod tests;
