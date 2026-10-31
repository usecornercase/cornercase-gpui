use gpui::{AnyElement, Context, Div, FontWeight, IntoElement, SharedString, Window, div, prelude::*, px};

use super::controls::Look;
use super::data::{Described, Item, Setting};
use super::pages::Slot;
use super::{Role, SettingsPanel, widgets};
use crate::notify::tip;
use crate::theme::{FONT_FAMILY, colours};
use crate::ui;

const MODE: f32 = 150.;
const NAME: f32 = 120.;
const LEAST: f32 = 120.;

pub(super) struct AgentItem<'d> {
    pub kind: &'d str,
    pub mode: Option<&'d Setting>,
    pub extra: Option<&'d Setting>,
}

fn head(text: &'static str) -> Div {
    div().text_size(px(10.5)).font_weight(FontWeight::SEMIBOLD).text_color(colours::faint()).child(text)
}

fn columns() -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(14.))
        .pt(px(12.))
        .pb(px(8.))
        .border_b_1()
        .border_color(colours::border())
        .child(head("AGENT").flex_grow().flex_basis(px(NAME)).min_w_0())
        .child(head("MODE").w(px(MODE)).flex_none())
        .child(head("EXTRA ARGUMENTS").w(px(Look::ARGUMENTS.width)).min_w(px(LEAST)))
        .child(div().w(px(widgets::SLOT)).flex_none())
}

fn agent_label(agent: &AgentItem<'_>) -> Div {
    let arguments = agent
        .mode
        .filter(|m| !m.is_default())
        .and_then(|m| m.choice(&m.text()))
        .filter(|c| !c.note.is_empty())
        .map(|c| {
            let ink = if c.dangerous { colours::failed() } else { colours::muted() };
            div().flex().font_family(FONT_FAMILY).text_size(px(11.)).text_color(ink).child(ui::one_line(c.note.clone()))
        });
    div()
        .flex_grow()
        .flex_basis(px(NAME))
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(div().font_weight(FontWeight::MEDIUM).child(agent.kind.to_string()))
        .children(arguments)
}

impl SettingsPanel {
    pub(super) fn agents_card(&self, d: &Described, items: &[Item], window: &Window, cx: &mut Context<Self>) -> Div {
        widgets::card().child(columns()).children(self.item_rows(d, items, window, cx))
    }

    pub(super) fn render_agent(&self, agent: &AgentItem<'_>, slot: Slot<'_>, cx: &mut Context<Self>) -> AnyElement {
        let extra =
            agent.extra.map(|e| self.render_text(&Role::Setting(e.key.clone()), Look::ARGUMENTS, slot.window, cx));
        let hint = agent.extra.and_then(|e| self.hint_of(&Role::Setting(e.key.clone()), e.hint(), slot.window, cx));
        let roles: Vec<Role> =
            [agent.mode, agent.extra].into_iter().flatten().map(|s| Role::Setting(s.key.clone())).collect();
        let row = div()
            .flex()
            .items_center()
            .gap(px(14.))
            .min_h(px(50.))
            .py(px(8.))
            .when(!slot.first, |d| d.border_t_1().border_color(colours::border()))
            .child(agent_label(agent))
            .child(div().w(px(MODE)).flex_none().flex().child(self.mode_control(agent, cx)))
            .child(div().w(px(Look::ARGUMENTS.width)).min_w(px(LEAST)).flex().children(extra))
            .child(div().w(px(widgets::SLOT)).flex_none().flex().justify_center().children(self.mode_saving(agent)));
        div()
            .flex()
            .flex_col()
            .child(row)
            .children(hint)
            .children(roles.iter().filter_map(|r| self.error_of(r)))
            .into_any_element()
    }

    fn mode_control(&self, agent: &AgentItem<'_>, cx: &mut Context<Self>) -> AnyElement {
        if let Some(mode) = agent.mode {
            return self.choice_button(mode, cx).flex_1().into_any_element();
        }
        div()
            .id(SharedString::from(format!("no-modes-{}", agent.kind)))
            .px(px(10.))
            .text_color(colours::faint())
            .tooltip(tip(format!("{} has no modes: give it extra arguments instead", agent.kind)))
            .child("—")
            .into_any_element()
    }

    fn mode_saving(&self, agent: &AgentItem<'_>) -> Option<AnyElement> {
        let mode = agent.mode?;
        self.busy.contains(&Role::Setting(mode.key.clone())).then(|| {
            ui::spinner(SharedString::from(format!("saving-{}", mode.key)), 13., colours::muted()).into_any_element()
        })
    }
}
