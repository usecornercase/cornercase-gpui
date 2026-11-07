use std::time::Instant;

use gpui::{
    AnyElement, App, Bounds, ClickEvent, Context, Div, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, Pixels, Point, SharedString, Size, Stateful, StatefulInteractiveElement as _, Styled, WeakEntity,
    Window, div, point, prelude::FluentBuilder as _, px, size,
};

use super::pane_accounts::{GAP, HEIGHT, LIFT, SIDE, TEXT, chars_across, cut, face, laid_out};
use super::pane_menu::pane_label;
use crate::app::Root;
use crate::model::{PaneGrid, Session};
use crate::render::Metrics;
use crate::render::layout::{Card, Layout};
use crate::sidebar::{cmd, tip};
use crate::store::{Busy, Store};
use crate::theme::{INACTIVE, colours, radius};
use crate::ui::{self, Icon, glass, icon};

const MORE_TEXT: f32 = 11.;
const BAND_PADDING: f32 = 4.;
const BAND_GAP: usize = 2;
const PILL_GAP: usize = 1;
const PILL_ROOM: usize = 2;
const ICON_ROOM: usize = 2;
const LEAST_TEXT: usize = 6;
const SEND_NOW: &str = "send now";
const WAIT_TURN: &str = "wait turn";
const TAKES_DRAFT: &str = "send now takes your line with it · needs usecornercase/cornercase-terminal#708";
const WHY: &str = "cornercase holds a message sent to this pane while you are typing, so the two do not merge.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HeldAct {
    SendNow,
    WaitTurn,
}

impl HeldAct {
    fn flag(self) -> &'static str {
        match self {
            Self::SendNow => "--send-now",
            Self::WaitTurn => "--wait-turn",
        }
    }

    pub(super) fn args(self, pane: u64) -> Vec<String> {
        cmd(&["held", "--pane", &pane.to_string(), self.flag()])
    }

    pub(super) fn busy(self, machine: Option<&str>, pane: u64) -> Busy {
        let label = match self {
            Self::SendNow => "Sending the held message…",
            Self::WaitTurn => "Letting the held message wait its turn…",
        };
        Busy::new(format!("held:{}:{pane}:{}", machine.unwrap_or_default(), self.flag()), label)
    }

    fn tip(self, held: &Held) -> &'static str {
        match self {
            Self::SendNow if held.draft => {
                "Send the first held message now, after what you typed: your unsent line goes with it."
            }
            Self::SendNow => "Send the first held message now.",
            Self::WaitTurn if held.queued => "You saw it: it goes right after you send yours.",
            Self::WaitTurn => "Say you saw it: it still goes right after you send yours.",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Held {
    text: String,
    queued: bool,
    draft: bool,
    running: Vec<HeldAct>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct BandFit {
    text: String,
    note: bool,
    wait: bool,
}

pub(crate) struct PaneNotes {
    pane: u64,
    below: u64,
    held: Option<Held>,
    opacity: f32,
}

pub(super) struct Room<'a> {
    pub layout: &'a Layout,
    pub metrics: &'a Metrics,
    pub taken: &'a [(u64, Pixels)],
    pub root: &'a WeakEntity<Root>,
}

pub(super) fn more_text(below: u64) -> String {
    format!("↓ {below} more")
}

pub(super) fn bottom_args(pane: u64) -> Vec<String> {
    cmd(&["scroll", "--pane", &pane.to_string(), "--bottom"])
}

pub(super) fn held_text(count: u32, from: Option<&str>, queued: bool) -> String {
    if queued {
        return match count {
            1 => "queued: goes right after yours".to_string(),
            n => format!("{n} queued: the first goes right after yours"),
        };
    }
    let waiting = match (count, from) {
        (1, None) => "1 message waiting".to_string(),
        (1, Some(from)) => format!("1 message waiting from {from}"),
        (n, None) => format!("{n} messages waiting"),
        (n, Some(from)) => format!("{n} messages waiting, the first from {from}"),
    };
    format!("{waiting} · goes after yours")
}

fn pill_room(label: &str) -> usize {
    label.chars().count() + PILL_ROOM
}

pub(super) fn band_fit(held: &Held, room: usize) -> Option<BandFit> {
    let send = pill_room(SEND_NOW);
    let turn = PILL_GAP + pill_room(WAIT_TURN) + if held.queued { ICON_ROOM } else { 0 };
    let left = room.checked_sub(send)?;
    let wait = left >= turn;
    let space = left - if wait { turn } else { 0 };
    let space = space.saturating_sub(BAND_GAP + if held.queued { ICON_ROOM } else { 0 });
    let note = held.draft && held.text.chars().count() + BAND_GAP + TAKES_DRAFT.chars().count() <= space;
    let text = if space >= LEAST_TEXT { cut(&held.text, space) } else { String::new() };
    Some(BandFit { text, note, wait })
}

pub(super) fn held_of(session: Option<&Session>, pane: u64) -> Option<Held> {
    let info = session?.pane(pane).filter(|info| info.held_sends > 0 && !info.resuming)?;
    let from = info.held_from.map(|sender| pane_label(session, sender));
    Some(Held {
        text: held_text(info.held_sends, from.as_deref(), info.held_queued),
        queued: info.held_queued,
        draft: info.draft,
        running: Vec::new(),
    })
}

fn running(store: &Store, machine: Option<&str>, pane: u64) -> Vec<HeldAct> {
    let now = Instant::now();
    let acts = [HeldAct::SendNow, HeldAct::WaitTurn];
    acts.into_iter().filter(|act| store.jobs.shown(act.busy(machine, pane).key(), now)).collect()
}

pub(super) fn notes_of(pane: &PaneGrid, held: Option<Held>, faded: bool) -> Option<PaneNotes> {
    let below = pane.below();
    let opacity = if faded { INACTIVE } else { 1. };
    (below > 0 || held.is_some()).then_some(PaneNotes { pane: pane.pane, below, held, opacity })
}

pub(super) fn band_bounds(card: Bounds<Pixels>, taken: Pixels) -> Bounds<Pixels> {
    let beside = if taken > px(0.) { taken + px(GAP) } else { px(0.) };
    let width = (card.size.width - px(SIDE * 2.) - beside).max(px(0.));
    let origin = point(card.left() + px(SIDE), card.bottom() - px(LIFT + HEIGHT));
    Bounds::new(origin, size(width, px(HEIGHT)))
}

pub(super) fn more_origin(last_row: Bounds<Pixels>, button: Size<Pixels>) -> Point<Pixels> {
    point((last_row.right() - button.width).max(last_row.left()), last_row.top())
}

fn held_words(pane: u64, held: &Held, fit: &BandFit) -> Stateful<Div> {
    let spins = held.queued && !fit.text.is_empty();
    div()
        .id(SharedString::from(format!("pane-held-{pane}")))
        .flex()
        .flex_1()
        .min_w_0()
        .items_center()
        .gap(px(GAP))
        .overflow_hidden()
        .tooltip(tip(format!("{}\n{WHY}", held.text)))
        .when(spins, |d| d.child(ui::spinner(("pane-held-queued", pane), TEXT, colours::muted())))
        .child(fit.text.clone())
        .when(fit.note, |d| d.child(div().text_color(colours::faint()).child(TAKES_DRAFT)))
}

fn held_pill(pane: u64, act: HeldAct, held: &Held, metrics: &Metrics) -> Stateful<Div> {
    face(metrics).id(SharedString::from(format!("pane-held{}-{pane}", act.flag()))).tooltip(tip(act.tip(held)))
}

fn pressing(pill: Stateful<Div>, pane: u64, act: HeldAct, root: &WeakEntity<Root>) -> Stateful<Div> {
    let root = root.clone();
    pill.cursor_pointer().on_click(move |_: &ClickEvent, _, cx| {
        let _ = root.update(cx, |this, cx| this.press_held(pane, act, cx));
    })
}

fn send_pill(pane: u64, held: &Held, room: &Room<'_>) -> Stateful<Div> {
    let act = HeldAct::SendNow;
    let pill = held_pill(pane, act, held, room.metrics).bg(glass::cobalt()).text_color(colours::on_brand());
    if held.running.contains(&act) {
        return pill.child(ui::spinner(("pane-held-sending", pane), TEXT, colours::on_brand())).child(SEND_NOW);
    }
    pressing(pill.hover(|s| s.bg(colours::brand_hover())).child(SEND_NOW), pane, act, room.root)
}

fn wait_pill(pane: u64, held: &Held, room: &Room<'_>) -> Stateful<Div> {
    let act = HeldAct::WaitTurn;
    let pill = held_pill(pane, act, held, room.metrics);
    if held.queued {
        let mark = icon(Icon::Check, TEXT).text_color(colours::text());
        return pill.bg(colours::selected()).text_color(colours::text()).child(mark).child(WAIT_TURN);
    }
    let pill = pill.bg(colours::raised()).text_color(colours::soft());
    if held.running.contains(&act) {
        return pill.child(ui::spinner(("pane-held-waiting", pane), TEXT, colours::soft())).child(WAIT_TURN);
    }
    pressing(pill.hover(|s| s.bg(colours::raised_hover())).child(WAIT_TURN), pane, act, room.root)
}

fn band(pane: u64, (held, fit): (&Held, &BandFit), width: Pixels, room: &Room<'_>) -> Div {
    let pills = div().flex().flex_none().items_center().gap(px(GAP)).child(send_pill(pane, held, room));
    div()
        .flex()
        .items_center()
        .gap(px(GAP))
        .w(width)
        .h(px(HEIGHT))
        .px(px(BAND_PADDING))
        .rounded(px(radius::CHIP))
        .bg(colours::selected())
        .overflow_hidden()
        .whitespace_nowrap()
        .font_family(room.metrics.family.clone())
        .text_size(px(TEXT))
        .line_height(px(HEIGHT))
        .text_color(colours::soft())
        .on_mouse_down(MouseButton::Left, |_: &MouseDownEvent, _, cx| cx.stop_propagation())
        .child(held_words(pane, held, fit))
        .child(pills.when(fit.wait, |d| d.child(wait_pill(pane, held, room))))
}

fn more_button((pane, below): (u64, u64), widest: Pixels, metrics: &Metrics, root: &WeakEntity<Root>) -> Stateful<Div> {
    let root = root.clone();
    div()
        .id(SharedString::from(format!("pane-more-{pane}")))
        .flex()
        .flex_none()
        .items_center()
        .max_w(widest)
        .overflow_hidden()
        .h(metrics.line_height)
        .px(px(6.))
        .rounded(px(radius::CHIP))
        .bg(colours::raised())
        .hover(|s| s.bg(colours::raised_hover()))
        .cursor_pointer()
        .whitespace_nowrap()
        .font_family(metrics.family.clone())
        .text_size(px(MORE_TEXT))
        .text_color(colours::soft())
        .tooltip(tip("Back to the bottom".to_string()))
        .on_mouse_down(MouseButton::Left, move |_: &MouseDownEvent, _, cx| {
            cx.stop_propagation();
            let _ = root.update(cx, |this, _| this.shown().queue.cmd(bottom_args(pane)));
        })
        .child(more_text(below))
}

impl PaneNotes {
    fn placed_band(&self, card: &Card, room: &Room<'_>, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
        let held = self.held.as_ref()?;
        let taken = room.taken.iter().find(|(pane, _)| *pane == self.pane).map_or(px(0.), |(_, width)| *width);
        let bounds = band_bounds(card.bounds, taken);
        let across = chars_across(f32::from(bounds.size.width) - BAND_PADDING * 2., room.metrics);
        let fit = band_fit(held, across)?;
        let band = band(self.pane, (held, &fit), bounds.size.width, room).opacity(self.opacity).into_any_element();
        Some(laid_out(band, |_| bounds.origin, window, cx).0)
    }

    fn placed_more(&self, card: &Card, room: &Room<'_>, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
        let below = Some(self.below).filter(|below| *below > 0)?;
        let row = card.rect.rows.saturating_sub(1);
        let last_row = room.layout.span(self.pane, row, (0, card.rect.cols))?;
        let button = more_button((self.pane, below), last_row.size.width, room.metrics, room.root);
        let button = button.opacity(self.opacity).into_any_element();
        Some(laid_out(button, |button| more_origin(last_row, button), window, cx).0)
    }
}

pub(super) fn place(notes: Vec<PaneNotes>, room: &Room<'_>, window: &mut Window, cx: &mut App) -> Vec<AnyElement> {
    let mut placed = Vec::new();
    for note in notes {
        let Some(card) = room.layout.card(note.pane) else { continue };
        placed.extend(note.placed_band(card, room, window, cx));
        placed.extend(note.placed_more(card, room, window, cx));
    }
    placed
}

impl Root {
    pub(super) fn pane_notes(&self, cx: &App) -> Vec<PaneNotes> {
        let session = self.shown_session().filter(|_| self.takes_input());
        let panes = &self.shown().grid.panes;
        let several = panes.len() > 1;
        let look = &self.side.look.current;
        let machine = self.machine.as_ref().map(|tab| tab.machine.as_str());
        let store = self.store.read(cx);
        let held_in = |pane: u64| {
            let running = running(store, machine, pane);
            held_of(session, pane).map(|held| Held { running, ..held })
        };
        let of = |p: &PaneGrid| notes_of(p, held_in(p.pane), look.fades(several, p.active));
        panes.iter().filter_map(of).collect()
    }

    fn press_held(&mut self, pane: u64, act: HeldAct, cx: &mut Context<Self>) {
        let machine = self.machine.as_ref().map(|tab| tab.machine.as_str());
        let busy = act.busy(machine, pane);
        if self.store.read(cx).busy(busy.key()) {
            return;
        }
        let queue = &self.shown().queue;
        self.store.update(cx, |s, cx| s.cmd_busy_on(queue, busy, act.args(pane), cx));
    }
}

#[cfg(test)]
mod tests;
