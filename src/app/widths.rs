use gpui::Context;

use super::keeper::{Keeper, Widths};
use super::{DOCK, DockWidths, GAP, PAIR, Root, SIDEBAR, TERMINAL_KEEP, WIDE_DOCK};
use crate::sidebar::rail::COLUMN;
use crate::ui::split::Split;

pub(super) fn restored(saved: &Widths, window_width: f32) -> (Split, Split, DockWidths) {
    let main = window_width - COLUMN;
    let sidebar = SIDEBAR.restored(saved.sidebar, main - TERMINAL_KEEP);
    let pair = PAIR.restored(saved.pair, main - TERMINAL_KEEP);
    let beside = main - sidebar.size().min(pair.chosen().unwrap_or(f32::MAX)) - GAP - TERMINAL_KEEP;
    let docks = DockWidths {
        panel: DOCK.restored(saved.panel, beside),
        browser: WIDE_DOCK.restored(saved.browser, beside),
        flow: WIDE_DOCK.restored(saved.flow, beside),
    };
    (sidebar, pair, docks)
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
