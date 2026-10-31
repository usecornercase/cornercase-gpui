use gpui::{
    AnyElement, App, ClickEvent, Context, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    Rgba, SharedString, StatefulInteractiveElement as _, Styled, Window, div, prelude::FluentBuilder as _, px,
};
use serde_json::Value;

use super::data::{Click, Entry, State, live_pane, notice, status_label};
use super::time::{Civil, local, now, relative, when_label};
use super::{QueuePanel, open_pane, starting};
use crate::model::Session;
use crate::notify::tip;
use crate::store::Store;
use crate::theme::colours;
use crate::ui::{self, Icon, Tone};

const WHEN: f32 = 124.;
const TOOL: f32 = 26.;

pub(super) struct Snapshot {
    pub(super) loaded: bool,
    pub(super) session: Option<Session>,
    boot: Option<String>,
    today: Civil,
    now: i64,
}

impl Snapshot {
    pub(super) fn new(app: Option<&Value>, session: Option<Session>) -> Self {
        let now = now();
        Self {
            loaded: app.is_some(),
            session,
            boot: app.and_then(|a| a.get("boot")).and_then(Value::as_str).map(str::to_string),
            today: local(now).unwrap_or(Civil::EPOCH),
            now,
        }
    }
}

struct Shown {
    state: State,
    pane: Option<u64>,
    click: Click,
    starting: bool,
    editing: bool,
}

fn state_colour(state: State) -> Rgba {
    match state {
        State::Pending => colours::accent(),
        State::Starting => colours::working(),
        State::Started => colours::done(),
        State::Failed => colours::failed(),
        State::Missed => colours::waiting(),
        State::Other => colours::muted(),
    }
}

fn glyph(state: State) -> Icon {
    match state {
        State::Pending => Icon::Pending,
        State::Starting => Icon::Working,
        State::Started => Icon::Done,
        State::Failed => Icon::Failed,
        State::Missed => Icon::Waiting,
        State::Other => Icon::Idle,
    }
}

fn state_icon(id: u64, state: State) -> Div {
    let slot = div().w(px(14.)).flex_none().pt(px(2.));
    if state == State::Starting {
        return slot.child(ui::spinner(("queue-spin", id), 14., state_colour(state)));
    }
    slot.child(ui::icon(glyph(state), 14.).text_color(state_colour(state)))
}

fn when_column(entry: &Entry, snapshot: &Snapshot) -> Div {
    let when = local(entry.at).map_or_else(|| entry.at.to_string(), |at| when_label(at, snapshot.today));
    div()
        .w(px(WHEN))
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(3.))
        .child(div().text_size(px(12.5)).text_color(colours::text()).child(when))
        .child(ui::dim(relative(entry.at, snapshot.now)))
}

fn entry_text(entry: &Entry, shown: &Shown, session: Option<&Session>) -> Div {
    let first_line = entry.first_prompt_line();
    let title = if shown.state == State::Started { colours::soft() } else { colours::text() };
    div()
        .flex_1()
        .min_w(px(0.))
        .flex()
        .flex_col()
        .gap(px(3.))
        .child(
            div()
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(title)
                .child(ui::one_line(entry.label.clone())),
        )
        .when(!first_line.is_empty() && first_line != entry.label, |d| {
            d.child(div().text_size(px(12.)).text_color(colours::muted()).child(ui::one_line(first_line)))
        })
        .child(meta_line(entry, shown, session))
        .children(notice(entry).map(|text| notice_line(text, shown.state)))
}

fn meta_line(entry: &Entry, shown: &Shown, session: Option<&Session>) -> Div {
    div()
        .flex()
        .gap(px(6.))
        .text_size(px(11.))
        .child(div().flex_none().text_color(state_colour(shown.state)).child(status_label(entry, shown.pane)))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .text_color(colours::faint())
                .child(ui::one_line(format!("· {}", entry.meta(session)))),
        )
}

fn notice_line(text: String, state: State) -> Div {
    let colour = state_colour(state);
    div()
        .mt(px(2.))
        .flex()
        .items_start()
        .gap(px(6.))
        .text_size(px(11.5))
        .text_color(colour)
        .child(ui::icon(Icon::Alert, 12.).text_color(colour).mt(px(1.)))
        .child(div().flex_1().min_w(px(0.)).child(text))
}

fn revealed(group: SharedString, actions: Div) -> Div {
    div().flex_none().invisible().group_hover(group, Styled::visible).child(actions)
}

fn start_button(entry: &Entry, cx: &mut Context<QueuePanel>) -> AnyElement {
    let started = entry.clone();
    ui::icon_text_button(("queue-run", entry.id), Icon::Play, "Start now", Tone::Primary)
        .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
            cx.stop_propagation();
            QueuePanel::run_now(&started, cx);
        }))
        .into_any_element()
}

fn edit_button(entry: &Entry, cx: &mut Context<QueuePanel>) -> AnyElement {
    let edited = entry.clone();
    ui::tool_button(("queue-edit", entry.id), Icon::Pencil, TOOL, false)
        .tooltip(tip("Edit"))
        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
            cx.stop_propagation();
            this.open_form(Some(&edited), window, cx);
        }))
        .into_any_element()
}

fn open_button(id: u64, pane: u64) -> AnyElement {
    ui::button(("queue-open", id), "Open")
        .tooltip(tip(format!("Show pane {pane}")))
        .on_click(move |_: &ClickEvent, _, cx| {
            cx.stop_propagation();
            open_pane(pane, cx);
        })
        .into_any_element()
}

fn remove_button(id: u64, state: State, cx: &mut Context<QueuePanel>) -> AnyElement {
    let text = if state == State::Started { "Remove it from the list: its agent keeps running" } else { "Remove" };
    ui::tool_button(("queue-rm", id), Icon::Trash, TOOL, false)
        .tooltip(tip(text))
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            cx.stop_propagation();
            this.remove(id, cx);
        }))
        .into_any_element()
}

fn actions(entry: &Entry, shown: &Shown, group: SharedString, cx: &mut Context<QueuePanel>) -> AnyElement {
    if shown.starting {
        return ui::busy_button(("queue-starting", entry.id), "Starting…", Tone::Primary).into_any_element();
    }
    let runnable = entry.runnable();
    let buttons = div()
        .flex()
        .items_center()
        .gap(px(4.))
        .children(runnable.then(|| start_button(entry, cx)))
        .children((runnable && !shown.editing).then(|| edit_button(entry, cx)))
        .children(shown.pane.map(|pane| open_button(entry.id, pane)))
        .children((shown.state != State::Starting).then(|| remove_button(entry.id, shown.state, cx)));
    if shown.editing { buttons.flex_none().into_any_element() } else { revealed(group, buttons).into_any_element() }
}

impl QueuePanel {
    fn shown(&self, entry: &Entry, snapshot: &Snapshot, cx: &App) -> Shown {
        let state = State::of(entry);
        let pane = live_pane(entry, snapshot.boot.as_deref(), snapshot.session.as_ref());
        let starting = Store::get(cx).read(cx).busy(&starting(entry.id));
        let editing = self.form.as_ref().is_some_and(|f| f.edit == Some(entry.id));
        Shown { state, pane, click: Click::of(entry, pane, starting || editing), starting, editing }
    }

    fn clicked(&mut self, click: Click, entry: &Entry, window: &mut Window, cx: &mut Context<Self>) {
        match click {
            Click::Edit => self.open_form(Some(entry), window, cx),
            Click::Open(pane) => open_pane(pane, cx),
            Click::Nothing => {}
        }
    }

    pub(super) fn entry_row(&self, entry: &Entry, snapshot: &Snapshot, cx: &mut Context<Self>) -> AnyElement {
        let shown = self.shown(entry, snapshot, cx);
        let group = SharedString::from(format!("queue-row-{}", entry.id));
        let (click, clicked) = (shown.click, entry.clone());
        ui::row(("queue", entry.id), shown.editing)
            .group(group.clone())
            .items_start()
            .gap(px(12.))
            .px(px(12.))
            .py(px(10.))
            .when(click == Click::Nothing, Styled::cursor_default)
            .child(when_column(entry, snapshot))
            .child(state_icon(entry.id, shown.state))
            .child(entry_text(entry, &shown, snapshot.session.as_ref()))
            .child(actions(entry, &shown, group, cx))
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.clicked(click, &clicked, window, cx)))
            .into_any_element()
    }
}
