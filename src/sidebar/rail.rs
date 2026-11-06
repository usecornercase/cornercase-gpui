use gpui::{
    AnyElement, ClickEvent, Context, Div, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent,
    ParentElement as _, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, div,
    prelude::FluentBuilder as _, px,
};

use super::menus::run_item;
use super::{card, tip};
use crate::app::{GAP, Root, TITLEBAR_HEIGHT};
use crate::context_menu::Entry;
use crate::dialogs;
use crate::keymap::{Command, with_keys};
use crate::panels::PanelKind;
use crate::panels::plugins::{Button, Run, run_action};
use crate::theme::colours;
use crate::ui::glass::ring;
use crate::ui::{self, Icon};

const RAIL_WIDTH: f32 = 44.;
const SLOT: f32 = 34.;
const RING: f32 = 3.;
const BUTTON_GAP: f32 = 4.;
pub(super) const COLUMN: f32 = RAIL_WIDTH + 2. * GAP;

const DOCKED: [PanelKind; 5] =
    [PanelKind::Changes, PanelKind::Files, PanelKind::Todo, PanelKind::Recap, PanelKind::Browser];
const SHEETS: [PanelKind; 11] = [
    PanelKind::Search,
    PanelKind::Issues,
    PanelKind::Queue,
    PanelKind::Flows,
    PanelKind::Usage,
    PanelKind::Accounts,
    PanelKind::Machines,
    PanelKind::Processes,
    PanelKind::Ports,
    PanelKind::Worktrees,
    PanelKind::Plugins,
];

fn tours_button() -> impl IntoElement {
    ui::tool_button("rail-tours", Icon::Help, SLOT, false)
        .tooltip(tip("Tours".to_string()))
        .on_click(|_: &ClickEvent, window, cx| crate::tour::open_catalogue(window, cx))
}

fn shortcuts_button() -> impl IntoElement {
    ui::tool_button("rail-shortcuts", Icon::Keyboard, SLOT, false)
        .tooltip(tip(with_keys("Keyboard shortcuts", Command::Shortcuts)))
        .on_click(|_: &ClickEvent, window, cx| dialogs::shortcuts(window, cx))
}

fn opens_menu(slot: Stateful<Div>, entries: Vec<Entry>, cx: &mut Context<Root>) -> Stateful<Div> {
    slot.on_mouse_down(
        MouseButton::Left,
        cx.listener(move |this, e: &MouseDownEvent, window, cx| {
            cx.stop_propagation();
            this.open_menu(e.position, entries.clone(), window, cx);
        }),
    )
}

fn plugin_slot(button: Button, tips: bool, cx: &mut Context<Root>) -> AnyElement {
    let id = SharedString::from(format!("rail-plugin-{}", button.plugin));
    let running = button.running(cx);
    let slot = if running { ui::busy_tool(id, SLOT) } else { ui::letter_button(id, button.glyph(), SLOT) };
    let slot = slot.when(tips, |slot| slot.tooltip(tip(button.tip())));
    match <[Run; 1]>::try_from(button.runs) {
        Ok(_) if running => slot,
        Ok([run]) => slot.on_click(cx.listener(move |_, _: &ClickEvent, _, cx| run_action(&run, cx, |_, _| {}))),
        Err(runs) => opens_menu(slot, runs.into_iter().map(run_item).collect(), cx),
    }
    .into_any_element()
}

impl Root {
    pub(crate) fn render_rail(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div().h_full().flex_none().flex().flex_col().pt(px(TITLEBAR_HEIGHT)).px(px(GAP)).pb(px(GAP)).child(
            card()
                .w(px(RAIL_WIDTH))
                .min_h_0()
                .items_center()
                .gap(px(BUTTON_GAP))
                .pb(px(8.))
                .child(self.rail_panels(cx))
                .child(tours_button())
                .child(shortcuts_button())
                .children(self.rail_buttons(&[PanelKind::Settings], cx)),
        )
    }

    fn rail_panels(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .id("rail-panels")
            .flex_1()
            .min_h_0()
            .w_full()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(BUTTON_GAP))
            .pt(px(5.))
            .pb(px(RING))
            .overflow_y_scroll()
            .children(self.rail_buttons(&self.docked_kinds(cx), cx))
            .children(self.plugin_slots(cx))
            .child(div().w(px(18.)).h(px(1.)).my(px(4.)).flex_none().rounded_full().bg(colours::edge()))
            .children(self.rail_buttons(&SHEETS, cx))
    }

    fn plugin_slots(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let buttons = self.store.read(cx).plugins.buttons();
        let tips = self.side.menu.is_none();
        buttons.into_iter().map(|button| plugin_slot(button, tips, cx)).collect()
    }

    fn docked_kinds(&self, cx: &Context<Self>) -> Vec<PanelKind> {
        let session = self.store.read(cx).session.as_ref();
        DOCKED.into_iter().chain(Some(PanelKind::Flow).filter(|kind| kind.shows(session))).collect()
    }

    fn rail_buttons(&self, kinds: &[PanelKind], cx: &mut Context<Self>) -> Vec<impl IntoElement + use<>> {
        let (dock, sheet) = {
            let s = self.store.read(cx);
            (s.shown_dock(), s.sheet)
        };
        let pointed = crate::tour::Guide::pointed(cx);
        kinds
            .iter()
            .map(|&kind| {
                let store = self.store.clone();
                let active = dock == Some(kind) || sheet == Some(kind);
                ui::tool_button(SharedString::from(format!("rail-{}", kind.title())), kind.icon(), SLOT, active)
                    .tooltip(tip(with_keys(kind.title(), Command::Panel(kind))))
                    .on_click(move |_: &ClickEvent, _, cx| store.update(cx, |s, cx| s.toggle(kind, cx)))
                    .when(pointed == Some(kind), |b| b.shadow(ring(colours::brand())))
            })
            .collect()
    }
}
