use std::rc::Rc;

use gpui::{Context, Div, ParentElement as _, Window};

use super::super::diagram::{OnRole, Plan, board_width, draw};
use super::super::flow_page::block;
use super::super::graph;
use super::super::model::FlowSpec;
use super::{FlowEditor, Open, View};
use crate::ui::Icon;

const TWO_COLUMNS: f32 = 1_000.;
const PLAN_MIN: f32 = 0.42;
const PLAN_MAX: f32 = 0.58;
const ABOUT_ROW: f32 = 560.;
pub(in crate::panels::flows) const PLAN_PAD: f32 = 16.;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::panels::flows) struct Layout {
    pub form: f32,
    pub plan: Option<f32>,
}

impl Layout {
    pub(in crate::panels::flows) fn about_in_a_row(self) -> bool {
        self.form >= ABOUT_ROW
    }
}

fn plan_width(editor: f32, narrowest: f32) -> f32 {
    (board_width(narrowest) + 2. * PLAN_PAD).clamp(editor * PLAN_MIN, editor * PLAN_MAX)
}

pub(in crate::panels::flows) fn layout(editor: f32, narrowest: f32) -> Layout {
    if editor < TWO_COLUMNS {
        return Layout { form: editor, plan: None };
    }
    let plan = plan_width(editor, narrowest);
    Layout { form: editor - plan, plan: Some(plan) }
}

fn box_target(plan: &FlowSpec, names: &[(u64, String)], name: &str) -> Option<Open> {
    if plan.is_lead(name) {
        return Some(Open::Lead);
    }
    names.iter().find(|(_, current)| current == name).map(|(id, _)| Open::Role(*id))
}

impl FlowEditor {
    pub(in crate::panels::flows) fn narrowest(&self) -> f32 {
        graph::fit(&self.plan, None, 0.).width
    }

    pub(in crate::panels::flows) fn plan_column(&self, cx: &Context<Self>) -> Div {
        let editor = cx.entity().downgrade();
        let on_role: OnRole = Rc::new(move |name, window, cx| {
            let _ = editor.update(cx, |this, cx| this.click_box(name, window, cx));
        });
        let focus = self.focus_name(cx);
        let plan =
            Plan { flow: &self.plan, run: None, focus: focus.as_deref(), key: "flow-draft", on_role: Some(&on_role) };
        block(Icon::Flows, "Plan").child(draw(plan, &self.board, &self.played))
    }

    fn focus_name(&self, cx: &Context<Self>) -> Option<String> {
        match self.open? {
            Open::Lead => Some(self.plan.lead_name().to_string()),
            Open::Role(id) => self.roles.iter().find(|role| role.id == id).map(|role| role.current_name(cx)),
        }
    }

    fn click_box(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let names: Vec<(u64, String)> = self.roles.iter().map(|role| (role.id, role.current_name(cx))).collect();
        let Some(target) = box_target(&self.plan, &names, name) else { return };
        if self.open == Some(target) {
            self.close_role(cx);
            return;
        }
        self.view = View::Form;
        self.open_role(target, window, cx);
    }
}

#[cfg(test)]
mod tests;
