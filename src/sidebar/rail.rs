use gpui::{
    AnyElement, ClickEvent, Context, Div, Entity, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent,
    ParentElement as _, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _, px,
};

use super::menus::run_item;
use super::{card, tip};
use crate::app::part::{self, Drawn};
use crate::app::{GAP, Root, TITLEBAR_HEIGHT};
use crate::context_menu::Entry;
use crate::keymap::{Command, with_keys};
use crate::panels::plugins::{Button, Run, run_action};
use crate::panels::{PanelKind, todo};
use crate::store::Store;
use crate::theme::colours;
use crate::ui::glass::ring;
use crate::ui::{self, Icon};

mod gear;
pub(crate) mod key;

const RAIL_WIDTH: f32 = 44.;
pub(super) const SLOT: f32 = 34.;
const RING: f32 = 3.;
const BUTTON_GAP: f32 = 6.;
const FOOT: f32 = 10.;
const RULE_GAP: f32 = 6.;
const COUNT: f32 = 13.;
const DOT: f32 = 6.;
pub(crate) const COLUMN: f32 = RAIL_WIDTH + 2. * GAP;

const DOCKED: [PanelKind; 5] =
    [PanelKind::Changes, PanelKind::Files, PanelKind::Todo, PanelKind::Recap, PanelKind::Browser];
const SHEETS: [PanelKind; 4] = [PanelKind::Search, PanelKind::Issues, PanelKind::Queue, PanelKind::Flows];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Slot {
    Panel(PanelKind),
    Plugins,
    Divider,
}

fn column(flow: bool) -> Vec<Slot> {
    DOCKED
        .into_iter()
        .chain(flow.then_some(PanelKind::Flow))
        .map(Slot::Panel)
        .chain([Slot::Plugins, Slot::Divider])
        .chain(SHEETS.into_iter().map(Slot::Panel))
        .collect()
}

#[derive(Clone, Copy)]
struct Shared {
    dock: Option<PanelKind>,
    sheet: Option<PanelKind>,
    pointed: Option<PanelKind>,
    tips: bool,
    notes: Notes,
}

fn tours_button(tips: bool) -> impl IntoElement {
    ui::tool_button("rail-tours", Icon::Help, SLOT, false)
        .when(tips, |b| b.tooltip(tip("Help".to_string())))
        .on_click(|_: &ClickEvent, window, cx| crate::tour::open_catalogue(window, cx))
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

fn plugin_slot(button: Button, says: Option<String>, cx: &mut Context<Root>) -> AnyElement {
    let id = SharedString::from(format!("rail-plugin-{}", button.plugin));
    let running = button.running(cx);
    let slot = if running { ui::busy_tool(id, SLOT) } else { ui::letter_button(id, button.glyph(), SLOT) };
    let slot = slot.when_some(says, |slot, says| slot.tooltip(tip(says)));
    match <[Run; 1]>::try_from(button.runs) {
        Ok(_) if running => slot,
        Ok([run]) => slot.on_click(cx.listener(move |_, _: &ClickEvent, _, cx| run_action(&run, cx, |_, _| {}))),
        Err(runs) => opens_menu(slot, runs.into_iter().map(run_item).collect(), cx),
    }
    .into_any_element()
}

impl Root {
    pub(crate) fn rail_part(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> Drawn {
        Drawn { element: self.render_rail(cx).into_any_element(), counted: false }
    }

    pub(crate) fn rail_slot(&self) -> impl IntoElement + use<> {
        div().w(px(COLUMN)).h_full().flex_none().child(part::cached(&self.parts.rail))
    }

    fn render_rail(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let shared = self.rail_shared(cx);
        let ringed = shared.pointed.map(gear::spot) == Some(gear::Spot::Gear);
        let gear = gear::gear_button((gear::gear_lit(shared.sheet), ringed, shared.tips), cx);
        div().h_full().flex_none().flex().flex_col().pt(px(TITLEBAR_HEIGHT)).px(px(GAP)).pb(px(GAP)).child(
            card()
                .w(px(RAIL_WIDTH))
                .min_h_0()
                .items_center()
                .gap(px(BUTTON_GAP))
                .pb(px(FOOT))
                .child(self.rail_panels(shared, cx))
                .child(tours_button(shared.tips))
                .child(gear),
        )
    }

    fn rail_panels(&self, shared: Shared, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let flow = PanelKind::Flow.shows(self.store.read(cx).session.as_ref());
        let slots: Vec<AnyElement> = column(flow)
            .into_iter()
            .flat_map(|slot| match slot {
                Slot::Panel(kind) => vec![rail_button(kind, shared, self.store.clone()).into_any_element()],
                Slot::Plugins => self.plugin_slots(cx),
                Slot::Divider => vec![rule().into_any_element()],
            })
            .collect();
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
            .children(slots)
    }

    fn plugin_slots(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let buttons = self.store.read(cx).plugins.buttons();
        let tips = self.side.menu.is_none();
        let machine = self.side.machine_place.as_ref().map(|place| place.machine.clone());
        buttons
            .into_iter()
            .map(|button| {
                let says = tips.then(|| button.tip(machine.as_deref()));
                plugin_slot(button, says, cx)
            })
            .collect()
    }

    fn rail_notes(&self, cx: &Context<Self>) -> Notes {
        let app = self.store.read(cx).app.as_ref();
        Notes {
            changed: self.shown_workspace().map_or(0, |(_, ws)| ws.changed_files()),
            todo: self.side.look.current.todo_dot && todo::any_pending(app),
        }
    }

    fn rail_shared(&self, cx: &mut Context<Self>) -> Shared {
        let (dock, sheet) = {
            let s = self.store.read(cx);
            (s.shown_dock(), s.sheet)
        };
        Shared {
            dock,
            sheet,
            pointed: crate::tour::Guide::pointed(cx),
            tips: self.side.menu.is_none(),
            notes: self.rail_notes(cx),
        }
    }
}

fn rule() -> Div {
    div().w(px(18.)).h(px(1.)).my(px(RULE_GAP)).flex_none().rounded_full().bg(colours::edge())
}

fn rail_button(kind: PanelKind, shared: Shared, store: Entity<Store>) -> impl IntoElement {
    let Shared { dock, sheet, pointed, tips, notes } = shared;
    let active = dock == Some(kind) || sheet == Some(kind);
    let ringed = pointed.map(gear::spot) == Some(gear::Spot::Icon(kind));
    ui::tool_button(SharedString::from(format!("rail-{}", kind.title())), kind.icon(), SLOT, active)
        .relative()
        .when(tips, |b| b.tooltip(tip(with_keys(&notes.named(kind), Command::Panel(kind)))))
        .on_click(move |_: &ClickEvent, _, cx| store.update(cx, |s, cx| s.toggle(kind, cx)))
        .when(ringed, |b| b.shadow(ring(colours::brand())))
        .children(notes.mark(kind))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Notes {
    changed: u64,
    todo: bool,
}

fn count_text(files: u64) -> Option<String> {
    match files {
        0 => None,
        1..=9 => Some(files.to_string()),
        _ => Some("9+".to_string()),
    }
}

fn corner() -> Div {
    div().absolute().right(px(2.)).bottom(px(2.)).flex().items_center().justify_center().rounded_full()
}

impl Notes {
    fn named(self, kind: PanelKind) -> String {
        match (kind, self.changed) {
            (PanelKind::Changes, 1) => format!("{} · 1 file", kind.title()),
            (PanelKind::Changes, files) if files > 1 => format!("{} · {files} files", kind.title()),
            _ => kind.title().to_string(),
        }
    }

    fn mark(self, kind: PanelKind) -> Option<Div> {
        match kind {
            PanelKind::Changes => count_text(self.changed).map(|text| {
                corner()
                    .min_w(px(COUNT))
                    .h(px(COUNT))
                    .px(px(2.))
                    .bg(colours::raised_hover())
                    .text_size(px(9.))
                    .line_height(px(COUNT))
                    .text_color(colours::text())
                    .child(text)
            }),
            PanelKind::Todo if self.todo => Some(corner().size(px(DOT)).m(px(2.)).bg(colours::muted())),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
