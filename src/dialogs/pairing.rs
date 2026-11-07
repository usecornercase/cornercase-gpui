use gpui::{
    AnyElement, App, AppContext as _, ClickEvent, ClipboardItem, Context, Div, Entity, FocusHandle,
    InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, Subscription, Window, div, px,
};

use super::{body_height, close, frame, muted, show};
use crate::cli::Stream;
use crate::store::{self, Store, ToastKind};
use crate::theme::colours;
use crate::ui::{self, Icon, Tone};

mod answer;

pub(crate) use answer::PairKey;
use answer::{Linked, Reading};

const ASKING: &str = "Asking cornercase for a code…";
const WAITING: &str = "Waiting for your phone…";
const ROOM: f32 = 280.;

struct PairingDialog {
    key: PairKey,
    focus: FocusHandle,
    reading: Reading,
    stream: Option<Stream>,
    left: bool,
    _store: Subscription,
}

pub(crate) fn pairing(key: PairKey, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| PairingDialog::new(key, cx));
    let focus = view.read(cx).focus.clone();
    show(view.into(), &focus, window, cx);
}

impl PairingDialog {
    fn new(key: PairKey, cx: &mut Context<Self>) -> Self {
        let store = Store::get(cx);
        let follow = cx.observe(&store, |this: &mut Self, store, cx| this.follow(&store, cx));
        let focus = cx.focus_handle();
        let mut dialog = Self { key, focus, reading: Reading::Asking, stream: None, left: false, _store: follow };
        dialog.ask(cx);
        dialog
    }

    fn ask(&mut self, cx: &mut Context<Self>) {
        self.reading = Reading::Asking;
        self.stream = store::follow_until(
            cx,
            self.key.args(),
            |this: &mut Self, line, cx| this.line(&line, cx),
            |this, stderr, cx| this.ended(&stderr, cx),
        );
        cx.notify();
    }

    fn follow(&mut self, store: &Entity<Store>, cx: &mut Context<Self>) {
        if store.read(cx).dialog.as_ref().is_none_or(|dialog| dialog.entity_id() != cx.entity_id()) {
            self.leave();
        }
    }

    fn leave(&mut self) {
        self.left = true;
        self.stream = None;
    }

    fn line(&mut self, line: &str, cx: &mut Context<Self>) {
        if self.left {
            return;
        }
        if let Some(connected) = self.reading.line(self.key, line) {
            self.leave();
            Store::get(cx).update(cx, |s, cx| {
                s.close_dialog(cx);
                s.toast(connected, ToastKind::Done, cx);
            });
        }
        cx.notify();
    }

    fn ended(&mut self, stderr: &str, cx: &mut Context<Self>) {
        if !self.left {
            self.stream = None;
            self.reading.ended(stderr);
            cx.notify();
        }
    }

    fn buttons(&self, cx: &mut Context<Self>) -> Div {
        let linked = if let Reading::Showing(linked) = &self.reading { Some(linked) } else { None };
        let again = matches!(self.reading, Reading::Refused(_)).then(|| {
            ui::button("pairing-again", "Try again").on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.ask(cx)))
        });
        div()
            .flex_1()
            .flex()
            .items_center()
            .gap(px(8.))
            .children(linked.map(|linked| copy_button(linked.url.clone())))
            .children(linked.map(|linked| open_button(linked.url.clone())))
            .child(div().flex_1())
            .children(again)
            .child(ui::primary_button("pairing-done", "Done").on_click(|_: &ClickEvent, _, cx| close(cx)))
    }
}

fn copy_button(url: String) -> Stateful<Div> {
    ui::icon_text_button("pairing-copy", Icon::Copy, "Copy link", Tone::Plain).on_click(move |_: &ClickEvent, _, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string(url.clone()));
        Store::get(cx).update(cx, |s, cx| s.toast("Copied the link", ToastKind::Info, cx));
    })
}

fn open_button(url: String) -> Stateful<Div> {
    ui::icon_text_button("pairing-open", Icon::External, "Open link", Tone::Plain)
        .on_click(move |_: &ClickEvent, _, cx| cx.open_url(&url))
}

fn linked_body(linked: &Linked, window: &Window) -> Div {
    let drawn = linked.code.clone().map(|matrix| ui::qr::code(matrix, ROOM, window));
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(div().flex().justify_center().pb(px(8.)).children(drawn))
        .child(div().text_size(px(12.)).text_color(colours::text()).child(SharedString::from(linked.url.clone())))
        .children(linked.notes.iter().map(|note| muted(note.clone())))
        .child(ui::busy("pairing-waiting", WAITING).pt(px(6.)))
}

fn starting_body(lines: &[String]) -> Div {
    let (first, rest) = lines.split_first().map_or(("", lines), |(first, rest)| (first.as_str(), rest));
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(ui::busy("pairing-starting", first.to_string()))
        .children(rest.iter().map(|line| muted(line.clone())))
}

fn body(reading: &Reading, window: &Window) -> AnyElement {
    match reading {
        Reading::Asking => ui::busy("pairing-asking", ASKING).into_any_element(),
        Reading::Starting(lines) => starting_body(lines).into_any_element(),
        Reading::Showing(linked) => linked_body(linked, window).into_any_element(),
        Reading::Refused(why) => ui::error_line(why.clone()).into_any_element(),
    }
}

impl Render for PairingDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let height = body_height(f32::from(window.viewport_size().height));
        let scrolled =
            div().id("pairing-body").max_h(px(height)).overflow_y_scroll().child(body(&self.reading, window));
        div()
            .track_focus(&self.focus)
            .on_key_down(|e: &KeyDownEvent, _, cx| {
                if e.keystroke.key == "enter" {
                    close(cx);
                }
            })
            .child(frame(self.key.title(), scrolled, self.buttons(cx)))
    }
}
