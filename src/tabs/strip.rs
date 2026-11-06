use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    App, ClickEvent, Context, Div, InteractiveElement, IntoElement, ParentElement, ScrollHandle, Stateful,
    StatefulInteractiveElement, Styled, WeakEntity, canvas, div, point, prelude::FluentBuilder as _, px,
};

use super::TAB_HEIGHT;
use crate::app::Root;
use crate::sidebar::tip;
use crate::theme::{colours, radius};

const ARROW_WIDTH: f32 = 20.;
pub(super) const ARROW_GAP: f32 = 2.;
const NEAR: f32 = 0.5;
const ARROWS_ROOM: f32 = 2. * (ARROW_WIDTH + ARROW_GAP) - 1.;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Spot {
    pub workspace: Option<u64>,
    pub tab: Option<u64>,
    pub width: f32,
    pub scrolled: f32,
    pub kept: bool,
}

#[derive(Clone, Default)]
pub(crate) struct TabStrip {
    handle: ScrollHandle,
    arrows: Rc<Cell<bool>>,
    wanted: Rc<Cell<(Spot, Option<usize>)>>,
    followed: Rc<Cell<Spot>>,
}

pub(super) fn overflows(max_offset: f32, arrows: bool) -> bool {
    max_offset > if arrows { ARROWS_ROOM } else { 0. }
}

pub(super) fn stepped(items: &[(f32, f32)], width: f32, scrolled: f32, forward: bool) -> f32 {
    if forward {
        let next = items.iter().find(|(_, right)| *right > scrolled + width + NEAR);
        next.map_or(scrolled, |(_, right)| right - width)
    } else {
        let next = items.iter().rev().find(|(left, _)| *left < scrolled - NEAR);
        next.map_or(scrolled, |(left, _)| *left)
    }
}

pub(super) fn can_step(scrolled: f32, max_offset: f32, forward: bool) -> bool {
    if forward { scrolled < max_offset - NEAR } else { scrolled > NEAR }
}

pub(super) fn in_view((left, right): (f32, f32), width: f32, scrolled: f32) -> f32 {
    if left < scrolled || right - left > width {
        left
    } else if right > scrolled + width {
        right - width
    } else {
        scrolled
    }
}

pub(super) fn followed_scroll(was: Spot, now: Spot, item: Option<(f32, f32)>) -> f32 {
    let other_workspace = was.workspace != now.workspace;
    let from = if other_workspace { 0. } else { now.scrolled };
    let left_alone = was.kept && (was.scrolled - now.scrolled).abs() <= NEAR;
    let moved = other_workspace || was.tab != now.tab || (was.width - now.width).abs() > NEAR || left_alone;
    item.filter(|_| moved).map_or(from, |item| in_view(item, now.width, from))
}

impl TabStrip {
    pub(super) fn handle(&self) -> &ScrollHandle {
        &self.handle
    }

    pub(super) fn arrows(&self) -> bool {
        let arrows = overflows(self.max_offset(), self.arrows.get());
        self.arrows.set(arrows);
        arrows
    }

    pub(super) fn show(&self, workspace: Option<u64>, tab: Option<u64>, index: Option<usize>) {
        self.wanted.set((Spot { workspace, tab, ..Spot::default() }, index));
    }

    pub(super) fn step(&self, forward: bool) {
        let items: Vec<(f32, f32)> = (0..self.handle.children_count()).filter_map(|i| self.item(i)).collect();
        self.scroll_to(stepped(&items, self.width(), self.scrolled(), forward));
    }

    pub(super) fn can_step(&self, forward: bool) -> bool {
        can_step(self.scrolled(), self.max_offset(), forward)
    }

    fn item(&self, index: usize) -> Option<(f32, f32)> {
        let start = self.handle.bounds().left();
        let bounds = self.handle.bounds_for_item(index)?;
        Some((f32::from(bounds.left() - start), f32::from(bounds.right() - start)))
    }

    fn scroll_to(&self, scrolled: f32) {
        self.handle.set_offset(point(px(-scrolled), px(0.)));
    }

    fn scrolled(&self) -> f32 {
        -f32::from(self.handle.offset().x)
    }

    fn width(&self) -> f32 {
        f32::from(self.handle.bounds().size.width)
    }

    fn max_offset(&self) -> f32 {
        f32::from(self.handle.max_offset().width)
    }

    fn settle(&self) -> bool {
        let (wanted, index) = self.wanted.get();
        let (scrolled, width) = (self.scrolled(), self.width());
        let item = index.and_then(|i| self.item(i));
        let next = followed_scroll(self.followed.get(), Spot { width, scrolled, ..wanted }, item);
        let kept = item.is_some_and(|item| (in_view(item, width, next) - next).abs() <= NEAR);
        self.followed.set(Spot { width, scrolled: next, kept, ..wanted });
        let moved = (next - scrolled).abs() > NEAR;
        if moved {
            self.scroll_to(next);
        }
        moved || overflows(self.max_offset(), self.arrows.get()) != self.arrows.get()
    }
}

fn settle(strip: &TabStrip, root: WeakEntity<Root>, cx: &mut App) {
    if strip.settle() {
        cx.defer(move |cx| {
            let _ = root.update(cx, |_, cx| cx.notify());
        });
    }
}

pub(super) fn watcher(strip: &TabStrip, root: WeakEntity<Root>) -> impl IntoElement + use<> {
    let strip = strip.clone();
    canvas(move |_, _, cx| settle(&strip, root, cx), |_, (), _, _| {}).absolute().size(px(0.))
}

pub(super) fn arrow(forward: bool, enabled: bool, cx: &mut Context<Root>) -> Stateful<Div> {
    let (id, glyph, label) =
        if forward { ("tabs-later", "›", "Later tabs") } else { ("tabs-earlier", "‹", "Earlier tabs") };
    div()
        .id(id)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .w(px(ARROW_WIDTH))
        .h(px(TAB_HEIGHT))
        .rounded(px(radius::CONTROL))
        .text_size(px(16.))
        .text_color(if enabled { colours::muted() } else { colours::faint() })
        .when(enabled, |d| d.cursor_pointer().hover(|s| s.bg(colours::hover())))
        .tooltip(tip(label.to_string()))
        .child(glyph)
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.side.strip.step(forward);
            cx.notify();
        }))
}

#[cfg(test)]
mod tests;
