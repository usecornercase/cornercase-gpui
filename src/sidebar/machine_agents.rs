use gpui::{
    AnyElement, Context, Div, InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement,
    SharedString, Stateful, StatefulInteractiveElement, Styled, div, prelude::FluentBuilder as _, px,
};

use super::look::Look;
use super::machine_rows::{DOWN_OPACITY, machine_tag, pick_on_click, row_end, status_of};
use super::rows::{End, chevron, hover_button, hover_listener};
use super::workspace::{agent_details, agent_text};
use crate::app::Root;
use crate::model::{MachinePlace, ThereAgent, ThereRow};
use crate::theme::colours;
use crate::ui;

const INDENT: f32 = 12.;
const FOLD: f32 = 10.;
const GAP: f32 = 4.;
const EDGES: f32 = 22.;
const ROW_SIDES: f32 = 12.;
const LEAD: f32 = FOLD + 14. + 3. * GAP;
const MIN_PLACE: f32 = 16.;
const ATTENTION: f32 = 14.;
const CHAR: f32 = 6.;
const TAG_ICON: f32 = 14.;
const TAG_LEAST_CHARS: usize = 3;

pub(super) struct ThereListing {
    pub place: Option<MachinePlace>,
    pub now: i64,
    pub card: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Fit {
    attention: bool,
    hidden: bool,
    machine: bool,
}

impl Fit {
    fn any(self) -> bool {
        let Self { attention, hidden, machine } = self;
        attention || hidden || machine
    }
}

fn chars_wide(chars: usize) -> f32 {
    f32::from(u16::try_from(chars).unwrap_or(u16::MAX)) * CHAR
}

fn indent(depth: usize) -> f32 {
    f32::from(u16::try_from(depth).unwrap_or(u16::MAX)) * INDENT
}

fn fit_tags(width: f32, attention: bool, hidden: usize, machine: &str) -> Fit {
    let mut left = (width - ROW_SIDES - LEAD - MIN_PLACE).min(width / 2.) + GAP;
    let mut take = |wanted: bool, size: f32| {
        let fits = wanted && size + GAP <= left;
        if fits {
            left -= size + GAP;
        }
        fits
    };
    let attention = take(attention, ATTENTION);
    let machine = take(true, TAG_ICON + chars_wide(machine.chars().count().min(TAG_LEAST_CHARS)));
    let hidden = take(machine && hidden > 0, chars_wide(1 + hidden.to_string().len()));
    Fit { attention, hidden, machine }
}

fn there_line(row: &ThereRow<'_>, place: Option<String>, look: &Look, now: i64) -> String {
    let ThereAgent { seen, pane, .. } = row.agent;
    let what = seen.pane_note(pane, now).unwrap_or_else(|| agent_details(pane, None, look));
    place.into_iter().chain([what]).collect::<Vec<_>>().join(" · ")
}

fn row_id(row: &ThereRow<'_>) -> SharedString {
    SharedString::from(format!("section-there-{}", row.fold_aim().key()))
}

fn still_lead(id: SharedString) -> Stateful<Div> {
    div().id(id).w(px(FOLD)).flex_none()
}

fn with_menu(frame: Stateful<Div>, row: &ThereRow<'_>, cx: &mut Context<Root>) -> Stateful<Div> {
    if row.children == 0 {
        return frame;
    }
    let aim = row.fold_aim();
    frame.on_mouse_down(
        MouseButton::Right,
        cx.listener(move |this, e: &MouseDownEvent, window, cx| {
            this.open_there_agent_menu(&aim, e.position, window, cx);
        }),
    )
}

fn menu_button(row: &ThereRow<'_>, gid: &SharedString, cx: &mut Context<Root>) -> Stateful<Div> {
    let aim = row.fold_aim();
    let id = SharedString::from(format!("{gid}-menu"));
    hover_button(id, gid.clone(), "⋯", 0).on_mouse_down(
        MouseButton::Left,
        cx.listener(move |this, e: &MouseDownEvent, window, cx| {
            cx.stop_propagation();
            this.open_there_agent_menu(&aim, e.position, window, cx);
        }),
    )
}

impl Root {
    pub(super) fn there_agent(
        &self,
        row: &ThereRow<'_>,
        place: Option<String>,
        listing: &ThereListing,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let ThereAgent { seen, tab, pane, .. } = row.agent;
        let gid = row_id(row);
        let lit = listing.place.as_ref().is_some_and(|shown| row.lit_by(shown));
        let title = pane.agent_title().unwrap_or_else(|| tab.name.clone());
        let line = there_line(row, place, &self.side.look.current, listing.now);
        if seen.pane_note_counts() {
            self.parts.counted.set(true);
        }
        let frame = ui::row(gid.clone(), lit)
            .group(gid.clone())
            .gap(px(GAP))
            .ml(px(indent(row.depth)))
            .px(px(6.))
            .py(px(3.))
            .when(!seen.linked(), |d| d.opacity(DOWN_OPACITY))
            .child(self.there_fold(row, &gid, cx))
            .child(status_of(seen, &self.parts.spots, pane.status.as_deref()))
            .child(agent_text(title, line, lit))
            .child(self.there_agent_end(row, listing.card, &gid, cx))
            .on_hover(hover_listener(gid, cx));
        pick_on_click(with_menu(frame, row, cx), row.agent.place(), cx).into_any_element()
    }

    fn there_fold(&self, row: &ThereRow<'_>, gid: &SharedString, cx: &mut Context<Self>) -> Stateful<Div> {
        let id = SharedString::from(format!("{gid}-fold"));
        if row.children == 0 {
            return still_lead(id);
        }
        let aim = row.fold_aim();
        if self.store.read(cx).folding_on_machine(&aim) {
            let spin = ui::spinner(SharedString::from(format!("{gid}-folding")), FOLD, colours::faint());
            return still_lead(id)
                .child(spin)
                .on_mouse_down(MouseButton::Left, |_: &MouseDownEvent, _, cx| cx.stop_propagation());
        }
        chevron(id, !row.agent.pane.collapsed).w(px(FOLD)).on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                cx.stop_propagation();
                this.store.update(cx, |s, cx| s.fold_on_machine(aim.clone(), cx));
            }),
        )
    }

    fn there_agent_end(&self, row: &ThereRow<'_>, card: f32, gid: &SharedString, cx: &mut Context<Self>) -> Div {
        let seen = row.agent.seen;
        let attention = row.attention();
        let hidden = row.hidden.len();
        let fit = fit_tags(card - EDGES - indent(row.depth), attention.is_some(), hidden, &seen.name);
        let end = End { gap: GAP, buttons: u8::from(row.children > 0), figures: fit.any() };
        let hovered = self.row_hovered(gid);
        row_end(end, hovered)
            .when(fit.attention, |d| d.child(status_of(seen, &self.parts.spots, attention)))
            .when(fit.hidden, |d| d.child(ui::dim(format!("+{hidden}")).flex_none()))
            .when(fit.machine, |d| d.child(machine_tag(seen)))
            .when(hovered && row.children > 0, |d| d.child(menu_button(row, gid, cx)))
    }
}

#[cfg(test)]
mod tests;
