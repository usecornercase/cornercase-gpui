use gpui::{
    ClickEvent, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, Rgba, SharedString,
    StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _, px, rgb,
};

use super::model::{BackSpec, Edge, FlowSpec, Run, StageSpec};
use super::widgets::{dot, show_pane};
use crate::theme::colours;
use crate::ui;

const ROLE_COLOURS: [u32; 6] = [0x007a_a2f7, 0x00bb_9af7, 0x007d_cfff, 0x009e_ce6a, 0x0073_daca, 0x00e0_af68];

pub(super) fn role_colour(flow: &FlowSpec, role: &str) -> Rgba {
    match flow.colour_index(role) {
        Some(i) => rgb(ROLE_COLOURS[i % ROLE_COLOURS.len()]),
        None => colours::text(),
    }
}

#[derive(Clone, Copy)]
struct Plan<'a> {
    flow: &'a FlowSpec,
    run: Option<&'a Run>,
}

impl Plan<'_> {
    fn last_edge(self) -> Option<Edge> {
        self.run.and_then(|r| r.last.as_ref()).and_then(|s| s.edge)
    }

    fn lit(self) -> Rgba {
        self.run
            .and_then(|r| r.last.as_ref())
            .and_then(|s| s.from.as_deref())
            .map_or(colours::accent(), |from| role_colour(self.flow, from))
    }
}

pub(super) fn diagram(flow: &FlowSpec, run: Option<&Run>) -> Div {
    let plan = Plan { flow, run };
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(orchestrator_row(plan))
        .children(
            flow.stages.iter().enumerate().flat_map(|(i, stage)| [stage_row(plan, i, stage), stage_arrow(plan, i)]),
        )
        .children(run.and_then(|r| r.outside).and_then(outside_note))
        .child(end(flow, run.is_some_and(|r| r.ended)))
}

fn orchestrator_row(plan: Plan) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .pb(px(8.))
        .mb(px(6.))
        .border_b_1()
        .border_color(colours::border())
        .child(role_box(plan, &plan.flow.orchestrator.name, 0))
        .child(ui::dim("moves the work between the stages"))
}

fn stage_row(plan: Plan, i: usize, stage: &StageSpec) -> Div {
    let active = plan.run.is_some_and(|r| r.stage == Some(i));
    let boxes = div()
        .flex()
        .flex_wrap()
        .gap(px(8.))
        .children(stage.roles.iter().enumerate().map(|(j, role)| role_box(plan, role, (i + 1) * 100 + j)));
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .child(
            div()
                .w(px(18.))
                .flex_none()
                .text_size(px(11.))
                .text_color(if active { colours::text() } else { colours::faint() })
                .child(format!("{}", i + 1)),
        )
        .child(boxes)
        .children(stage.back.as_ref().map(|back| loop_label(plan, i, back)))
}

fn loop_label(plan: Plan, i: usize, back: &BackSpec) -> Div {
    let rounds = plan.run.and_then(|r| r.rounds.get(i).copied());
    let label = match rounds {
        Some(n) => format!("↺ {} · {n}/{} → stage {}", back.when, back.at_most, back.to),
        None => format!("↺ {} · at most {} → stage {}", back.when, back.at_most, back.to),
    };
    let colour = if rounds.is_some_and(|n| n > back.at_most) {
        colours::waiting()
    } else if plan.last_edge() == Some(Edge::Loop { stage: i }) {
        plan.lit()
    } else {
        colours::muted()
    };
    div()
        .px(px(8.))
        .py(px(2.))
        .rounded(px(10.))
        .border_1()
        .border_color(colours::border())
        .text_size(px(11.))
        .text_color(colour)
        .child(label)
}

fn stage_arrow(plan: Plan, i: usize) -> Div {
    let lit = plan.last_edge() == Some(Edge::Forward { stage: i });
    arrow(if lit { plan.lit() } else { colours::faint() })
}

fn outside_note(edge: Edge) -> Option<Div> {
    let Edge::Outside { from, to } = edge else { return None };
    let from = from.map_or("the start".to_string(), |f| format!("stage {}", f + 1));
    Some(
        div()
            .ml(px(28.))
            .my(px(4.))
            .px(px(8.))
            .py(px(2.))
            .rounded(px(5.))
            .border_1()
            .border_dashed()
            .border_color(colours::waiting())
            .text_size(px(11.))
            .text_color(colours::waiting())
            .child(format!("outside the plan: {from} → stage {}", to + 1)),
    )
}

fn end(flow: &FlowSpec, reached: bool) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .pl(px(28.))
        .text_color(if reached { colours::accent() } else { colours::faint() })
        .child("◆")
        .child(div().text_size(px(12.)).child(flow.end_label()))
        .when(reached, |d| d.child(ui::dim("· reached")))
}

fn arrow(colour: Rgba) -> Div {
    div().pl(px(44.)).h(px(18.)).text_size(px(13.)).text_color(colour).child("↓")
}

fn role_box(plan: Plan, role: &str, id: usize) -> impl IntoElement + use<> {
    let colour = role_colour(plan.flow, role);
    let agent = plan.flow.role(role).map(|r| match &r.model {
        Some(model) => format!("{} · {model}", r.agent),
        None => r.agent.clone(),
    });
    let member = plan.run.and_then(|r| r.member(role));
    let open = member.filter(|m| !m.closed);
    let status = open.and_then(|m| m.status.clone());
    let working = status.as_deref() == Some("working");
    let holder = plan.run.and_then(|r| r.holder.as_deref()) == Some(role);
    let name: SharedString = role.to_string().into();
    div()
        .id(("flow-box", id))
        .min_w(px(120.))
        .px(px(10.))
        .py(px(5.))
        .rounded(px(6.))
        .bg(if holder { colours::selected() } else { colours::terminal() })
        .when(working, |d| d.border_2().border_color(colour))
        .when(!working, |d| d.border_1().border_color(colours::border()))
        .when(member.is_some_and(|m| m.closed), |d| d.opacity(0.5))
        .when_some(open.and_then(|m| m.pane), |d, pane| {
            d.cursor_pointer()
                .hover(|s| s.bg(colours::hover()))
                .on_click(move |_: &ClickEvent, _, cx| show_pane(pane, cx))
        })
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(dot(colour))
                .child(div().font_weight(FontWeight::SEMIBOLD).text_size(px(12.)).text_color(colour).child(name))
                .when(plan.run.is_some(), |d| d.child(ui::badge(status.as_deref()))),
        )
        .children(agent.map(|a| ui::dim(a).pl(px(13.))))
}
