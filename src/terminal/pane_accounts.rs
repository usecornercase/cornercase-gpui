use std::time::Instant;

use gpui::{
    AnyElement, App, AvailableSpace, Bounds, Context, Div, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, ParentElement, Pixels, Point, SharedString, Size, Stateful, StatefulInteractiveElement as _,
    Styled, WeakEntity, Window, div, point, prelude::FluentBuilder as _, px,
};

use crate::app::Root;
use crate::context_menu::{Entry, above, choice, item, submenu};
use crate::model::PaneInfo;
use crate::model::accounts::{Switch, Switcher, switching_key};
use crate::panels::PanelKind;
use crate::render::Metrics;
use crate::render::layout::Layout;
use crate::sidebar::tip;
use crate::store::Store;
use crate::theme::{INACTIVE, colours, radius};
use crate::ui::{self, Icon, icon};

pub(super) const TEXT: f32 = 10.;
pub(super) const HEIGHT: f32 = 12.;
pub(super) const LIFT: f32 = 1.;
pub(super) const SIDE: f32 = 12.;
pub(super) const GAP: f32 = 4.;
const DIM: f32 = 0.6;
const AROUND: usize = 4;
const LEAST_NAME: usize = 3;
const WAITING: &str = "restarts after this turn";

pub(crate) struct PaneChips {
    pane: u64,
    switchers: Vec<Switcher>,
    tips: Vec<String>,
    doing: Option<String>,
    opacity: f32,
}

fn named(switcher: &Switcher) -> String {
    if switcher.waiting { format!("{} · {WAITING}", switcher.name) } else { switcher.name.clone() }
}

fn with_agent(switcher: &Switcher) -> String {
    format!("{}: {}", switcher.agent, named(switcher))
}

fn fitting(texts: impl Iterator<Item = String>, room: usize) -> Vec<String> {
    let mut used = 0;
    let mut kept = Vec::new();
    for text in texts {
        let wanted = text.chars().count() + AROUND + usize::from(!kept.is_empty());
        if used + wanted > room {
            break;
        }
        used += wanted;
        kept.push(text);
    }
    kept
}

pub(super) fn cut(text: &str, chars: usize) -> String {
    if text.chars().count() <= chars {
        return text.to_string();
    }
    let kept: String = text.chars().take(chars.saturating_sub(1)).collect();
    format!("{kept}…")
}

pub(super) fn chip_texts(switchers: &[Switcher], room: usize) -> Vec<String> {
    let whole = fitting(switchers.iter().map(with_agent), room);
    if whole.len() == switchers.len() {
        return whole;
    }
    let short = fitting(switchers.iter().map(named), room);
    if !short.is_empty() {
        return short;
    }
    let space = room.saturating_sub(AROUND);
    switchers.first().filter(|_| space >= LEAST_NAME).map(|s| cut(&named(s), space)).into_iter().collect()
}

pub(super) fn chips_origin(card: Bounds<Pixels>, row: Size<Pixels>) -> Point<Pixels> {
    point(card.right() - px(SIDE) - row.width, card.bottom() - px(LIFT) - row.height)
}

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "a width in characters, floored")]
pub(super) fn chars_across(width: f32, metrics: &Metrics) -> usize {
    let char_width = f32::from(metrics.cell_width) / f32::from(metrics.font_size) * TEXT;
    (width / char_width).floor().max(0.) as usize
}

pub(super) fn room_in(card: Bounds<Pixels>, metrics: &Metrics) -> usize {
    chars_across(f32::from(card.size.width) - SIDE * 2., metrics)
}

pub(super) fn laid_out(
    mut element: AnyElement,
    at: impl FnOnce(Size<Pixels>) -> Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) -> (AnyElement, Size<Pixels>) {
    let size = element.layout_as_root(AvailableSpace::min_size(), window, cx);
    element.prepaint_at(at(size), window, cx);
    (element, size)
}

pub(super) fn place(
    chips: Vec<PaneChips>,
    (layout, metrics): (&Layout, &Metrics),
    root: &WeakEntity<Root>,
    window: &mut Window,
    cx: &mut App,
) -> (Vec<AnyElement>, Vec<(u64, Pixels)>) {
    let mut placed = Vec::new();
    let mut taken = Vec::new();
    for pane in chips {
        let Some(card) = layout.card(pane.pane).map(|card| card.bounds) else { continue };
        let Some(row) = pane.row(room_in(card, metrics), metrics, root) else { continue };
        let (row, size) = laid_out(row, |size| chips_origin(card, size), window, cx);
        placed.push(row);
        taken.push((pane.pane, size.width));
    }
    (placed, taken)
}

pub(super) fn face(metrics: &Metrics) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(3.))
        .h(px(HEIGHT))
        .px(px(4.))
        .rounded(px(radius::chip()))
        .whitespace_nowrap()
        .font_family(metrics.family.clone())
        .text_size(px(TEXT))
        .line_height(px(HEIGHT))
        .text_color(colours::muted())
}

impl PaneChips {
    fn row(&self, across: usize, metrics: &Metrics, root: &WeakEntity<Root>) -> Option<AnyElement> {
        let row = div().flex().items_center().gap(px(GAP)).opacity(self.opacity);
        if let Some(doing) = &self.doing {
            let text = cut(doing, across.saturating_sub(AROUND));
            let spinner = ui::spinner(("pane-account-busy", self.pane), TEXT, colours::muted());
            return Some(row.child(face(metrics).child(spinner).child(text)).into_any_element());
        }
        let texts = chip_texts(&self.switchers, across);
        if texts.is_empty() {
            return None;
        }
        let chips = texts.into_iter().enumerate().map(|(i, text)| self.chip(i, text, metrics, root));
        Some(row.children(chips).into_any_element())
    }

    fn chip(&self, index: usize, text: String, metrics: &Metrics, root: &WeakEntity<Root>) -> Stateful<Div> {
        let pane = self.pane;
        let agent = self.switchers[index].agent.clone();
        let root = root.clone();
        face(metrics)
            .id(SharedString::from(format!("pane-account-{pane}-{agent}")))
            .cursor_pointer()
            .hover(|s| s.bg(colours::hover()))
            .when_some(self.tips.get(index).cloned(), |d, text| d.tooltip(tip(text)))
            .on_mouse_down(MouseButton::Left, move |e: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                let _ = root.update(cx, |this, cx| {
                    let entries = this.account_entries(pane, &agent, cx);
                    this.open_menu(above(e.position, &entries), entries, window, cx);
                });
            })
            .child(text)
            .child(icon(Icon::ChevronDown, 9.).text_color(colours::faint()))
    }
}

fn chips_of(pane: &PaneInfo, faded: bool, menu_open: bool, store: &Store) -> Option<PaneChips> {
    let switchers = store.accounts.switchers(pane);
    if switchers.is_empty() {
        return None;
    }
    let tips = if menu_open {
        Vec::new()
    } else {
        switchers.iter().map(|s| store.accounts.switcher_tip(s, &store.selection)).collect()
    };
    let key = switching_key(pane.id);
    let doing = store.jobs.doing(&key).filter(|_| store.jobs.shown(&key, Instant::now())).map(str::to_string);
    let dim = pane.agent.is_none() && !pane.at_prompt;
    let opacity = if faded { INACTIVE } else { 1. } * if dim { DIM } else { 1. };
    Some(PaneChips { pane: pane.id, switchers, tips, doing, opacity })
}

pub(super) fn switch_rows<A>(switches: Vec<Switch>, act: A) -> Vec<Entry>
where
    A: Fn(&mut Root, Switch, &mut Context<Root>) + Clone + 'static,
{
    let row = |switch: Switch| {
        let act = act.clone();
        choice(switch.label(), switch.chosen, move |this, _, cx| {
            if switch.does_anything() {
                act(this, switch.clone(), cx);
            }
        })
    };
    switches.into_iter().map(row).collect()
}

fn switch_here(this: &mut Root, switch: Switch, cx: &mut Context<Root>) {
    this.store.update(cx, |s, cx| s.switch_account(switch, cx));
}

fn manage_item() -> Entry {
    item("Manage accounts…", |this, _, cx| this.store.update(cx, |s, cx| s.open(PanelKind::Accounts, cx)))
}

impl Root {
    pub(super) fn pane_chips(&self, cx: &App) -> Vec<PaneChips> {
        let store = self.store.read(cx);
        let look = &self.side.look.current;
        let wanted = look.account_switcher && store.accounts.any();
        let Some(session) = self.session.as_ref().filter(|_| wanted && self.machine.is_none()) else {
            return Vec::new();
        };
        let (several, menu_open) = (self.own.grid.panes.len() > 1, self.side.menu.is_some());
        let shown =
            self.own.grid.panes.iter().filter_map(|p| Some((session.pane(p.pane)?, look.fades(several, p.active))));
        shown.filter_map(|(pane, faded)| chips_of(pane, faded, menu_open, store)).collect()
    }

    fn account_entries(&self, pane: u64, agent: &str, cx: &App) -> Vec<Entry> {
        let Some(info) = self.session.as_ref().and_then(|s| s.pane(pane)) else { return Vec::new() };
        let accounts = &self.store.read(cx).accounts;
        let switches = accounts.switches_of(info, Some(agent));
        if switches.is_empty() {
            return Vec::new();
        }
        let mut rows = switch_rows(switches, switch_here);
        rows.extend([Entry::Separator, manage_item()]);
        rows
    }

    pub(crate) fn switch_account_item(&self, pane: u64, cx: &App) -> Option<Entry> {
        let info = self.session.as_ref()?.pane(pane)?;
        let entries = self.account_entries(pane, info.agent.as_deref()?, cx);
        (!entries.is_empty()).then(|| submenu("Switch account", entries))
    }
}

#[cfg(test)]
mod tests;
