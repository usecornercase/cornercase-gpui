use gpui::{
    ClickEvent, Context, Div, InteractiveElement as _, IntoElement as _, ParentElement as _, Rgba, SharedString,
    Stateful, StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

use super::super::cards::{avatar, conductor};
use super::super::diagram::{role_colour, stage_ring};
use super::super::draft::{alone, first_line, stage_of};
use super::super::editor_view::{adder, choice, labelled, line, text_control};
use super::super::flow_page::{block, role_box, role_head};
use super::super::graph::Spot;
use super::super::model::RoleSpec;
use super::super::widgets::{eyebrow, icon_action};
use super::super::wires;
use super::{FlowEditor, Open, RoleFields, StageFields};
use crate::theme::colours;
use crate::ui::{self, Icon, Tone, boxed};

const TYPED_WAITS: &str = "Typed for now · needs usecornercase/cornercase-terminal#704";

fn peek(spec: &RoleSpec) -> Div {
    let text = first_line(&spec.prompt).unwrap_or("No prompt of its own").to_string();
    div().min_w_0().flex().text_size(px(11.5)).text_color(colours::faint()).child(ui::one_line(text))
}

impl FlowEditor {
    pub(in crate::panels::flows) fn team(&self, window: &Window, cx: &Context<Self>) -> Div {
        block(Icon::Accounts, "Team and stages")
            .child(self.lead(window, cx))
            .children(self.stages.iter().enumerate().map(|(i, s)| self.stage(i, s, window, cx)))
            .child(adder(
                "editor-add-stage",
                Icon::Plus,
                "Add a stage",
                false,
                cx.listener(|this, _: &ClickEvent, window, cx| this.add_stage(window, cx)),
            ))
    }

    pub(in crate::panels::flows) fn end(&self, cx: &Context<Self>) -> Div {
        let pick = |id: &'static str, label: &'static str, commits: bool| {
            choice(id, label, self.commits == commits, cx, move |this, _, cx| this.toggle_commits(commits, cx))
        };
        let head = div()
            .flex()
            .items_center()
            .gap(px(9.))
            .child(div().relative().size(px(22.)).flex_none().child(wires::diamond(
                Spot { x: 11., y: 11. },
                6.,
                colours::edge(),
            )))
            .child(eyebrow("When the work is done"));
        let segments = ui::segments(vec![
            pick("editor-commits", "Leave the commits", true),
            pick("editor-pr", "Open a pull request", false),
        ]);
        div().flex().flex_col().gap(px(8.)).child(head).child(div().ml(px(31.)).flex().child(segments))
    }

    fn lead(&self, window: &Window, cx: &Context<Self>) -> Stateful<Div> {
        if self.open == Some(Open::Lead) { self.lead_open(window, cx) } else { self.lead_closed(cx) }
    }

    fn lead_closed(&self, cx: &Context<Self>) -> Stateful<Div> {
        let mut spec = self.orchestrator.spec(cx);
        spec.name = self.plan.lead_name().to_string();
        role_box("editor-lead", colours::soft(), false)
            .cursor_pointer()
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.open_role(Open::Lead, window, cx)))
            .child(role_head((0, &spec), colours::soft(), false, Some(peek(&spec))))
    }

    fn lead_open(&self, window: &Window, cx: &Context<Self>) -> Stateful<Div> {
        let lead = &self.orchestrator;
        let top = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .child(conductor(30.))
            .child(div().flex_1().text_size(px(13.)).font_weight(gpui::FontWeight::SEMIBOLD).child("orchestrator"))
            .child(
                ui::tool_button("editor-lead-close", Icon::ChevronDown, 26., false)
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_role(cx))),
            );
        role_box("editor-lead", colours::soft(), true)
            .p(px(12.))
            .gap(px(10.))
            .child(top)
            .child(Self::agent_row(lead, window, cx))
            .child(ui::dim(TYPED_WAITS))
            .child(text_control("Prompt", &lead.prompt, window, cx))
            .child(Self::command(Open::Lead, lead, window, cx))
    }

    fn agent_row(role: &RoleFields, window: &Window, cx: &Context<Self>) -> Div {
        line()
            .child(labelled("Agent", &role.agent, window, cx))
            .child(labelled("Model", &role.model, window, cx))
            .child(labelled("Effort", &role.effort, window, cx))
    }

    fn stage(&self, index: usize, stage: &StageFields, window: &Window, cx: &Context<Self>) -> Div {
        let last = index + 1 == self.stages.len();
        let head = div()
            .flex()
            .items_center()
            .gap(px(9.))
            .child(stage_ring(index))
            .child(eyebrow(&format!("Stage {}", index + 1)).flex_1())
            .child(Self::stage_moves(index, last, cx));
        let body = div()
            .ml(px(11.))
            .pl(px(20.))
            .border_l_1()
            .border_color(colours::border())
            .flex()
            .flex_col()
            .gap(px(8.))
            .children(stage.roles.iter().filter_map(|id| self.role(*id, window, cx)))
            .child(adder(
                ("editor-add-beside", index),
                Icon::Plus,
                "Add a role beside",
                true,
                cx.listener(move |this, _: &ClickEvent, window, cx| this.add_role_beside(index, window, cx)),
            ))
            .children(Self::loop_part(index, stage, window, cx));
        div().flex().flex_col().gap(px(8.)).child(head).child(body)
    }

    fn stage_moves(index: usize, last: bool, cx: &Context<Self>) -> Div {
        let step = |id: &'static str, glyph: Icon, hint: &'static str, up: bool| {
            let blocked = if up { index == 0 } else { last };
            icon_action((id, index), glyph, hint, false)
                .when(blocked, |action| action.opacity(0.35).cursor_default())
                .when(!blocked, |action| {
                    action.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.move_stage(index, up, cx)))
                })
        };
        div()
            .flex()
            .flex_none()
            .gap(px(4.))
            .child(step("stage-up", Icon::ArrowUp, "Move this stage up", true))
            .child(step("stage-down", Icon::ArrowDown, "Move this stage down", false))
    }

    fn role(&self, id: u64, window: &Window, cx: &Context<Self>) -> Option<Stateful<Div>> {
        let place = self.roles.iter().position(|role| role.id == id)?;
        let role = &self.roles[place];
        let name = role.current_name(cx);
        let colour = role_colour(&self.plan, &name);
        if self.open == Some(Open::Role(id)) {
            return Some(self.role_open(role, colour, window, cx));
        }
        let mut spec = role.spec(cx);
        if spec.name.is_empty() {
            spec.name = "No name".into();
        }
        Some(
            role_box(("editor-role", id), colour, false)
                .cursor_pointer()
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    this.open_role(Open::Role(id), window, cx);
                }))
                .child(role_head((place + 1, &spec), colour, false, Some(peek(&spec)))),
        )
    }

    fn role_open(&self, role: &RoleFields, colour: Rgba, window: &Window, cx: &Context<Self>) -> Stateful<Div> {
        let id = role.id;
        let name = role.current_name(cx);
        role_box(("editor-role", id), colour, true)
            .p(px(12.))
            .gap(px(10.))
            .child(Self::role_top(role, &name, colour, window, cx))
            .child(Self::agent_row(role, window, cx))
            .child(ui::dim(TYPED_WAITS))
            .child(text_control("Prompt", &role.prompt, window, cx))
            .child(
                line()
                    .items_center()
                    .gap(px(18.))
                    .child(Self::toggle(role, false, cx))
                    .child(Self::toggle(role, true, cx)),
            )
            .child(self.works_in(id, cx))
            .child(Self::command(Open::Role(id), role, window, cx))
    }

    fn role_top(role: &RoleFields, name: &str, colour: Rgba, window: &Window, cx: &Context<Self>) -> Div {
        let id = role.id;
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(avatar(name, colour, 30.))
            .child(boxed(&role.name, window, cx).flex_1().min_w_0())
            .child(
                icon_action(("role-remove", id), Icon::Trash, "Remove this role", true)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.remove_role(id, cx))),
            )
            .child(
                ui::tool_button(("role-close", id), Icon::ChevronDown, 26., false)
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_role(cx))),
            )
    }

    fn toggle(role: &RoleFields, own: bool, cx: &Context<Self>) -> Stateful<Div> {
        let id = role.id;
        let (key, checked, label) = if own {
            ("role-own", role.own_worktree, "A worktree of its own")
        } else {
            ("role-edits", role.edits, "Edits the files")
        };
        ui::checkbox((key, id), checked, label)
            .text_size(px(12.))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_role(id, own, cx)))
    }

    fn works_in(&self, id: u64, cx: &Context<Self>) -> Div {
        let own = stage_of(&self.stages, id);
        let segments = (0..self.stages.len()).map(|k| Self::in_stage(id, k, own == Some(k), cx)).collect();
        let new_stage = if alone(&self.stages, id) {
            ui::resting_button(("role-new-stage", id), "New stage", Tone::Plain).into_any_element()
        } else {
            adder(
                ("role-new-stage", id),
                Icon::Plus,
                "New stage",
                true,
                cx.listener(move |this, _: &ClickEvent, window, cx| this.in_new_stage(id, window, cx)),
            )
            .into_any_element()
        };
        let row = div().flex().flex_wrap().items_center().gap(px(8.)).child(ui::segments(segments)).child(new_stage);
        ui::field("Works in", row)
    }

    fn in_stage(id: u64, stage: usize, selected: bool, cx: &Context<Self>) -> gpui::AnyElement {
        let key = (SharedString::from(format!("role-in-{id}")), stage);
        choice(key, format!("Stage {}", stage + 1), selected, cx, move |this, _, cx| this.move_role(id, stage, cx))
    }

    fn command(open: Open, role: &RoleFields, window: &Window, cx: &Context<Self>) -> Div {
        let key = match open {
            Open::Lead => 0,
            Open::Role(id) => id + 1,
        };
        let glyph = if role.command_open { Icon::ChevronDown } else { Icon::ChevronRight };
        let toggle = div()
            .id(("role-command", key))
            .flex()
            .items_center()
            .gap(px(6.))
            .text_size(px(12.))
            .text_color(colours::muted())
            .cursor_pointer()
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_command(open, cx)))
            .child(ui::icon(glyph, 12.))
            .child("Arguments and variables");
        let fields = line().child(labelled("Arguments", &role.args, window, cx)).child(labelled(
            "Variables",
            &role.env,
            window,
            cx,
        ));
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(div().flex().child(toggle))
            .children(role.command_open.then_some(fields))
    }
}
