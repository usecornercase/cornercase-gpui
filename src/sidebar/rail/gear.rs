use gpui::{
    Context, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent, Pixels, Point,
    StatefulInteractiveElement as _, Styled as _, Window, prelude::FluentBuilder as _,
};

use super::SLOT;
use crate::app::Root;
use crate::context_menu::{Entry, item_with_keys};
use crate::keymap::{self, Command};
use crate::panels::PanelKind;
use crate::platform::Os;
use crate::sidebar::tip;
use crate::theme::colours;
use crate::ui::glass::ring;
use crate::ui::{self, Icon};

const GEAR_ID: &str = "rail-gear";

const GEAR: [&[PanelKind]; 3] = [
    &[PanelKind::Settings],
    &[PanelKind::Accounts, PanelKind::Usage, PanelKind::Machines],
    &[PanelKind::Worktrees, PanelKind::Processes, PanelKind::Ports, PanelKind::Plugins],
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Spot {
    Icon(PanelKind),
    Gear,
}

pub(super) fn behind_gear(kind: PanelKind) -> bool {
    GEAR.iter().any(|group| group.contains(&kind))
}

pub(super) fn gear_lit(sheet: Option<PanelKind>) -> bool {
    sheet.is_some_and(behind_gear)
}

pub(super) fn spot(kind: PanelKind) -> Spot {
    if behind_gear(kind) { Spot::Gear } else { Spot::Icon(kind) }
}

fn gear_entries(os: Os) -> Vec<Entry> {
    let mut entries = Vec::new();
    for (at, group) in GEAR.iter().enumerate() {
        if at > 0 {
            entries.push(Entry::Separator);
        }
        for &kind in *group {
            entries.push(item_with_keys(
                kind.icon(),
                kind.title(),
                keymap::keys_of(Command::Panel(kind), os),
                move |root, _, cx| {
                    root.store.update(cx, |s, cx| s.open(kind, cx));
                },
            ));
        }
    }
    entries
}

impl Root {
    fn open_gear_menu(&mut self, pointer: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        self.open_menu_above_from((GEAR_ID, pointer), gear_entries(Os::CURRENT), window, cx);
    }
}

pub(super) fn gear_button((lit, ringed, tips): (bool, bool, bool), cx: &mut Context<Root>) -> impl IntoElement + use<> {
    ui::tool_button(GEAR_ID, Icon::Settings, SLOT, lit)
        .when(tips, |b| b.tooltip(tip("Settings and more")))
        .when(ringed, |b| b.shadow(ring(colours::brand())))
        .capture_any_mouse_down(cx.listener(|this, _, window, cx| {
            if this.menu_from(GEAR_ID) {
                this.close_menu(window, cx);
                cx.stop_propagation();
            }
        }))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, e: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                this.open_gear_menu(e.position, window, cx);
            }),
        )
}

#[cfg(test)]
mod tests;
