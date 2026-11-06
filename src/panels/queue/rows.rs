use gpui::{
    AnyElement, App, ClickEvent, Context, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    Rgba, SharedString, Stateful, StatefulInteractiveElement as _, Styled, Window, div, prelude::FluentBuilder as _,
    px,
};
use serde_json::Value;

use super::data::{Click, Entry, Row, State, live_pane, notice, remove_tip, status_label};
use super::time::{Civil, local, now, relative, when_label};
use super::{QueuePanel, open_pane, pausing, starting};
use crate::model::Session;
use crate::notify::tip;
use crate::store::Store;
use crate::theme::colours;
use crate::ui::{self, Icon, Tone};

const WHEN: f32 = 124.;
const TOOL: f32 = 26.;
const TEXT_KEEP: f32 = 100.;
pub(super) const RUN_INDENT: f32 = WHEN + 38.;
const PAUSE: &str = "Pause: it starts no run until you resume it";
const RESUME: &str = "Resume: it goes on from its next time, with no catch-up";

pub(super) struct Snapshot {
    pub(super) loaded: bool,
    pub(super) session: Option<Session>,
    boot: Option<String>,
    pub(super) today: Civil,
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

pub(super) struct Shown {
    pub(super) state: State,
    pub(super) pane: Option<u64>,
    pub(super) click: Click,
    starting: bool,
    pausing: bool,
    editing: bool,
}

fn state_colour(state: State) -> Rgba {
    match state {
        State::Pending | State::Repeating => colours::accent(),
        State::Starting => colours::working(),
        State::Started => colours::done(),
        State::Failed => colours::failed(),
        State::Missed | State::Kept => colours::waiting(),
        State::Removed | State::Paused | State::Other => colours::muted(),
    }
}

fn glyph(state: State) -> Icon {
    match state {
        State::Pending => Icon::Pending,
        State::Starting => Icon::Working,
        State::Started | State::Removed => Icon::Done,
        State::Failed => Icon::Failed,
        State::Missed | State::Kept => Icon::Waiting,
        State::Repeating => Icon::Repeat,
        State::Paused => Icon::Pause,
        State::Other => Icon::Idle,
    }
}

pub(super) fn state_icon(id: u64, state: State) -> Div {
    let slot = div().w(px(14.)).flex_none().pt(px(2.));
    if state == State::Starting {
        return slot.child(ui::spinner(("queue-spin", id), 14., state_colour(state)));
    }
    slot.child(ui::icon(glyph(state), 14.).text_color(state_colour(state)))
}

fn when_column(entry: &Entry, snapshot: &Snapshot) -> Div {
    let column = div().w(px(WHEN)).flex_none().flex().flex_col().gap(px(3.));
    if State::of(entry) == State::Paused {
        return column.child(div().text_size(px(12.5)).text_color(colours::muted()).child("paused"));
    }
    let at = entry.next_time();
    let when = local(at).map_or_else(|| at.to_string(), |at| when_label(at, snapshot.today));
    column
        .child(div().text_size(px(12.5)).text_color(colours::text()).child(when))
        .child(ui::dim(relative(at, snapshot.now)))
}

fn entry_text(entry: &Entry, shown: &Shown, session: Option<&Session>) -> Div {
    let first_line = entry.first_prompt_line();
    let quiet = matches!(shown.state, State::Started | State::Removed | State::Kept | State::Paused);
    let title = if quiet { colours::soft() } else { colours::text() };
    div()
        .flex_1()
        .min_w(px(TEXT_KEEP))
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
    div().flex().gap(px(6.)).text_size(px(11.)).child(state_text(entry, shown)).child(
        div()
            .flex_1()
            .min_w(px(0.))
            .text_color(colours::faint())
            .child(ui::one_line(format!("· {}", entry.meta(session)))),
    )
}

pub(super) fn state_text(entry: &Entry, shown: &Shown) -> Div {
    div().flex_none().text_color(state_colour(shown.state)).child(status_label(entry, shown.pane))
}

pub(super) fn notice_line(text: String, state: State) -> Div {
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

pub(super) fn revealed(group: SharedString, actions: Div) -> Div {
    div().flex_none().invisible().group_hover(group, Styled::visible).child(actions)
}

fn start_button(entry: &Entry, shown: &Shown, cx: &mut Context<QueuePanel>) -> AnyElement {
    if shown.starting {
        return ui::busy_button(("queue-starting", entry.id), "Starting…", Tone::Primary).into_any_element();
    }
    let started = entry.clone();
    ui::icon_text_button(("queue-run", entry.id), Icon::Play, "Start now", Tone::Primary)
        .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
            cx.stop_propagation();
            QueuePanel::run_now(&started, cx);
        }))
        .into_any_element()
}

fn tool_slot() -> Div {
    div().size(px(TOOL)).flex_none().flex().items_center().justify_center()
}

fn pause_button(entry: &Entry, shown: &Shown, cx: &mut Context<QueuePanel>) -> AnyElement {
    if shown.pausing {
        return tool_slot().child(ui::spinner(("queue-pausing", entry.id), 13., colours::muted())).into_any_element();
    }
    let (changed, paused) = (entry.clone(), entry.paused);
    let (id, glyph, text) =
        if paused { ("queue-resume", Icon::Repeat, RESUME) } else { ("queue-pause", Icon::Pause, PAUSE) };
    ui::tool_button((id, entry.id), glyph, TOOL, false)
        .tooltip(tip(text))
        .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
            cx.stop_propagation();
            QueuePanel::set_paused(&changed, !paused, cx);
        }))
        .into_any_element()
}

fn edit_button(entry: &Entry, shown: &Shown, cx: &mut Context<QueuePanel>) -> AnyElement {
    if shown.editing {
        return tool_slot().into_any_element();
    }
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

fn remove_button(entry: &Entry, cx: &mut Context<QueuePanel>) -> AnyElement {
    let id = entry.id;
    ui::tool_button(("queue-rm", id), Icon::Trash, TOOL, false)
        .tooltip(tip(remove_tip(entry)))
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            cx.stop_propagation();
            this.remove(id, cx);
        }))
        .into_any_element()
}

fn actions(row: &Row, shown: &Shown, group: SharedString, cx: &mut Context<QueuePanel>) -> AnyElement {
    let entry = row.entry;
    if shown.starting && !entry.is_routine() {
        return start_button(entry, shown, cx);
    }
    let runnable = entry.runnable();
    let buttons = div()
        .flex()
        .items_center()
        .gap(px(4.))
        .children(runnable.then(|| start_button(entry, shown, cx)))
        .children(entry.is_routine().then(|| pause_button(entry, shown, cx)))
        .children(runnable.then(|| edit_button(entry, shown, cx)))
        .children(shown.pane.map(|pane| open_button(entry.id, pane)))
        .children(row.removable().then(|| remove_button(entry, cx)));
    if shown.editing || shown.starting || shown.pausing {
        buttons.flex_none().into_any_element()
    } else {
        revealed(group, buttons).into_any_element()
    }
}

pub(super) fn run_actions(run: &Entry, shown: &Shown, group: SharedString, cx: &mut Context<QueuePanel>) -> Div {
    let buttons = div()
        .flex()
        .items_center()
        .gap(px(4.))
        .children(shown.pane.map(|pane| open_button(run.id, pane)))
        .children((shown.state != State::Starting).then(|| remove_button(run, cx)));
    revealed(group, buttons)
}

pub(super) fn base_row(id: u64, selected: bool, click: Click, group: SharedString) -> Stateful<Div> {
    ui::row(("queue", id), selected).group(group).items_start().when(click == Click::Nothing, Styled::cursor_default)
}

impl QueuePanel {
    pub(super) fn shown(&self, entry: &Entry, snapshot: &Snapshot, cx: &App) -> Shown {
        let state = State::of(entry);
        let pane = live_pane(entry, snapshot.boot.as_deref(), snapshot.session.as_ref());
        let store = Store::get(cx).read(cx);
        let (starting, pausing) = (store.busy(&starting(entry.id)), store.busy(&pausing(entry.id)));
        let editing = self.form.as_ref().is_some_and(|f| f.edit == Some(entry.id));
        Shown { state, pane, click: Click::of(entry, pane, starting || editing), starting, pausing, editing }
    }

    pub(super) fn clicked(&mut self, click: Click, entry: &Entry, window: &mut Window, cx: &mut Context<Self>) {
        match click {
            Click::Edit => self.open_form(Some(entry), window, cx),
            Click::Open(pane) => open_pane(pane, cx),
            Click::Nothing => {}
        }
    }

    pub(super) fn entry_row(&self, row: &Row, snapshot: &Snapshot, cx: &mut Context<Self>) -> AnyElement {
        let entry = row.entry;
        let shown = self.shown(entry, snapshot, cx);
        let group = SharedString::from(format!("queue-row-{}", entry.id));
        let (click, clicked) = (shown.click, entry.clone());
        base_row(entry.id, shown.editing, click, group.clone())
            .flex_wrap()
            .gap(px(12.))
            .px(px(12.))
            .pt(px(10.))
            .pb(px(if row.runs.is_empty() { 10. } else { 4. }))
            .child(when_column(entry, snapshot))
            .child(state_icon(entry.id, shown.state))
            .child(entry_text(entry, &shown, snapshot.session.as_ref()))
            .child(div().flex_none().ml_auto().child(actions(row, &shown, group, cx)))
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.clicked(click, &clicked, window, cx)))
            .into_any_element()
    }
}
