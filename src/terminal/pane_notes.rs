use gpui::{
    AnyElement, App, Bounds, Div, InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement, Pixels,
    Point, SharedString, Size, Stateful, StatefulInteractiveElement as _, Styled, WeakEntity, Window, div, point,
    prelude::FluentBuilder as _, px, size,
};

use super::pane_accounts::{GAP, HEIGHT, LIFT, SIDE, TEXT, chars_across, cut, laid_out};
use crate::app::Root;
use crate::model::{PaneGrid, PaneInfo};
use crate::render::Metrics;
use crate::render::layout::{Card, Layout};
use crate::sidebar::{cmd, tip};
use crate::theme::{INACTIVE, colours, radius};

const MORE_TEXT: f32 = 11.;
const BAND_PADDING: f32 = 4.;
const BAND_GAP: usize = 2;
const LEAST_TEXT: usize = 8;
const WAITS: &str = "Send now and Wait turn need usecornercase/cornercase-terminal#676";
const WHY: &str = "cornercase holds a message sent to this pane while you are typing, so the two do not merge.";

pub(crate) struct PaneNotes {
    pane: u64,
    below: u64,
    held: Option<String>,
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

pub(super) fn held_text(count: u32, draft: bool) -> String {
    let waiting = if count == 1 { "1 message waiting".to_string() } else { format!("{count} messages waiting") };
    let when = if draft { "goes after yours" } else { "goes out in a moment" };
    format!("{waiting} · {when}")
}

pub(super) fn band_texts(text: &str, room: usize) -> Option<(String, Option<&'static str>)> {
    let whole = text.chars().count() + BAND_GAP + WAITS.chars().count();
    if whole <= room {
        return Some((text.to_string(), Some(WAITS)));
    }
    (room >= LEAST_TEXT).then(|| (cut(text, room), None))
}

pub(super) fn notes_of(pane: &PaneGrid, info: Option<&PaneInfo>, faded: bool) -> Option<PaneNotes> {
    let below = pane.below();
    let held = info.filter(|i| i.held_sends > 0).map(|i| held_text(i.held_sends, i.draft));
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

fn band(pane: u64, (text, waits): (String, Option<&'static str>), width: Pixels, metrics: &Metrics) -> Stateful<Div> {
    div()
        .id(SharedString::from(format!("pane-held-{pane}")))
        .flex()
        .items_center()
        .justify_between()
        .w(width)
        .h(px(HEIGHT))
        .px(px(BAND_PADDING))
        .rounded(px(radius::CHIP))
        .bg(colours::selected())
        .overflow_hidden()
        .whitespace_nowrap()
        .font_family(metrics.family.clone())
        .text_size(px(TEXT))
        .line_height(px(HEIGHT))
        .text_color(colours::soft())
        .tooltip(tip(format!("{WHY}\n{WAITS}")))
        .child(text)
        .when_some(waits, |d, waits| d.child(div().text_color(colours::faint()).child(waits)))
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
            let _ = root.update(cx, |this, _| this.queue.cmd(bottom_args(pane)));
        })
        .child(more_text(below))
}

impl PaneNotes {
    fn placed_band(&self, card: &Card, room: &Room<'_>, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
        let held = self.held.as_deref()?;
        let taken = room.taken.iter().find(|(pane, _)| *pane == self.pane).map_or(px(0.), |(_, width)| *width);
        let bounds = band_bounds(card.bounds, taken);
        let across = chars_across(f32::from(bounds.size.width) - BAND_PADDING * 2., room.metrics);
        let texts = band_texts(held, across)?;
        let band = band(self.pane, texts, bounds.size.width, room.metrics).opacity(self.opacity).into_any_element();
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
    pub(super) fn pane_notes(&self) -> Vec<PaneNotes> {
        let session = self.session.as_ref();
        let several = self.grid.panes.len() > 1;
        let look = &self.side.look.current;
        let of = |p: &PaneGrid| notes_of(p, session.and_then(|s| s.pane(p.pane)), look.fades(several, p.active));
        self.grid.panes.iter().filter_map(of).collect()
    }
}

#[cfg(test)]
mod tests;
