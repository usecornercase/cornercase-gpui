use gpui::{Context, Window};

use super::keeper::{Keeper, Widths};
use super::{DOCK, DockWidths, GAP, PAIR, Root, SIDEBAR, TERMINAL_KEEP, WIDE_DOCK, wide_default};
use crate::panels::PanelKind;
use crate::sidebar::rail::COLUMN;
use crate::ui::split::Split;

pub(super) fn restored(saved: &Widths, window_width: f32) -> (Split, Split, DockWidths) {
    let main = window_width - COLUMN;
    let room = main - TERMINAL_KEEP - GAP - DOCK.min();
    let sidebar = SIDEBAR.restored(saved.sidebar, room);
    let pair = PAIR.restored(saved.pair, room);
    let docks = DockWidths {
        panel: DOCK.restored(saved.panel, f32::MAX),
        browser: WIDE_DOCK.restored(saved.browser, f32::MAX),
        flow: WIDE_DOCK.restored(saved.flow, f32::MAX),
    };
    (sidebar, pair, docks)
}

pub(super) fn dock_room(window_width: f32, sidebar_extent: f32) -> f32 {
    window_width - COLUMN - sidebar_extent - GAP - TERMINAL_KEEP
}

pub(super) fn dock_room_without_sidebar(window_width: f32) -> f32 {
    window_width - COLUMN - TERMINAL_KEEP
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Arranged {
    pub(super) sidebar: bool,
    pub(super) dock: f32,
}

pub(super) fn arranged(window_width: f32, sidebar_extent: f32, dock: Option<(Split, f32)>) -> Arranged {
    let Some((split, wanted)) = dock else { return Arranged { sidebar: true, dock: 0. } };
    let beside = dock_room(window_width, sidebar_extent);
    if beside >= split.min() {
        return Arranged { sidebar: true, dock: split.fitted(wanted, beside) };
    }
    Arranged { sidebar: false, dock: split.fitted(wanted, dock_room_without_sidebar(window_width)) }
}

pub(super) fn of(sidebar: Split, pair: Split, docks: DockWidths) -> Widths {
    Widths {
        sidebar: sidebar.chosen(),
        pair: pair.chosen(),
        panel: docks.panel.chosen(),
        browser: docks.browser.chosen(),
        flow: docks.flow.chosen(),
    }
}

impl Root {
    pub(super) fn arrangement(&self, kind: Option<PanelKind>, window: &Window) -> Arranged {
        let width = f32::from(window.viewport_size().width);
        let dock = kind.map(|kind| match self.dock_widths.wide(kind) {
            Some(wide) => (wide, wide.chosen().unwrap_or_else(|| wide_default(width))),
            None => (self.dock_widths.panel, self.dock_widths.panel.size()),
        });
        arranged(width, self.sidebar_extent(), dock)
    }

    pub(super) fn keep_widths(&self, cx: &mut Context<Self>) {
        let now = of(self.sidebar_width, self.pair_width, self.dock_widths);
        let keeper = Keeper::get(cx);
        if keeper.read(cx).seen_widths() != now {
            keeper.update(cx, |keeper, cx| keeper.widths_moved(now, cx));
        }
    }
}

#[cfg(test)]
mod tests;
