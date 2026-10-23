//! A flow's plan, drawn as the TUI's Flow panel draws it: the orchestrator on top, apart; the
//! stages top to bottom with parallel roles side by side; each loop beside its stage with its
//! label; the end at the bottom. For a run it lights up: a working role's box gets a heavy border
//! in its colour, the arrow the work last took the colour of the role it left, loop rounds count
//! (orange past their limit), and the end turns blue once reached. A click on a live box goes to
//! its pane.

use gpui::{
    ClickEvent, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, Rgba, SharedString,
    StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _, px, rgb,
};

use super::model::{Edge, FlowSpec, Run};
use crate::store::Store;
use crate::theme::colours;
use crate::ui;

/// The roles' colours: the group presets without red (failed) and orange (waiting).
const ROLE_COLOURS: [u32; 6] = [0x7aa2f7, 0xbb9af7, 0x7dcfff, 0x9ece6a, 0x73daca, 0xe0af68];

pub fn role_colour(flow: &FlowSpec, role: &str) -> Rgba {
    match flow.colour_index(role) {
        Some(i) => rgb(ROLE_COLOURS[i % ROLE_COLOURS.len()]),
        None => colours::text(),
    }
}

pub fn diagram(flow: &FlowSpec, run: Option<&Run>) -> Div {
    let mut body = div().flex().flex_col().gap(px(2.)).child(
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .pb(px(8.))
            .mb(px(6.))
            .border_b_1()
            .border_color(colours::border())
            .child(role_box(flow, &flow.orchestrator.name, run, 0))
            .child(ui::dim("moves the work between the stages")),
    );
    let last = run.and_then(|r| r.last.as_ref());
    let last_colour = last.and_then(|s| s.from.as_deref()).map(|from| role_colour(flow, from));
    for (i, stage) in flow.stages.iter().enumerate() {
        let active = run.is_some_and(|r| r.stage == Some(i));
        let boxes = div()
            .flex()
            .flex_wrap()
            .gap(px(8.))
            .children(stage.roles.iter().enumerate().map(|(j, role)| role_box(flow, role, run, (i + 1) * 100 + j)));
        let mut row = div()
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
            .child(boxes);
        if let Some(back) = &stage.back {
            let rounds = run.and_then(|r| r.rounds.get(i).copied());
            let label = match rounds {
                Some(n) => format!("↺ {} · {n}/{} → stage {}", back.when, back.at_most, back.to),
                None => format!("↺ {} · at most {} → stage {}", back.when, back.at_most, back.to),
            };
            let colour = if rounds.is_some_and(|n| n > back.at_most) {
                colours::waiting()
            } else if matches!(last.and_then(|s| s.edge), Some(Edge::Loop { stage }) if stage == i) {
                last_colour.unwrap_or(colours::accent())
            } else {
                colours::muted()
            };
            row = row.child(
                div()
                    .px(px(8.))
                    .py(px(2.))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(colours::border())
                    .text_size(px(11.))
                    .text_color(colour)
                    .child(label),
            );
        }
        body = body.child(row);
        let lit = matches!(last.and_then(|s| s.edge), Some(Edge::Forward { stage }) if stage == i);
        body = body.child(arrow(if lit { last_colour.unwrap_or(colours::accent()) } else { colours::faint() }));
    }
    if let Some(Some(Edge::Outside { from, to })) = run.map(|r| r.outside) {
        let from = from.map_or("the start".to_string(), |f| format!("stage {}", f + 1));
        body = body.child(
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
        );
    }
    let reached = run.is_some_and(|r| r.ended);
    body.child(
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .pl(px(28.))
            .text_color(if reached { colours::accent() } else { colours::faint() })
            .child("◆")
            .child(div().text_size(px(12.)).child(flow.end_label()))
            .when(reached, |d| d.child(ui::dim("· reached"))),
    )
}

/// The arrow down to the next stage (or to the end).
fn arrow(colour: Rgba) -> Div {
    div().pl(px(44.)).h(px(18.)).text_size(px(13.)).text_color(colour).child("↓")
}

/// A role's box: its name in its colour, its agent and model, and live its state.
fn role_box(flow: &FlowSpec, role: &str, run: Option<&Run>, id: usize) -> impl IntoElement + use<> {
    let colour = role_colour(flow, role);
    let spec = flow.role(role);
    let agent = spec.map(|r| match &r.model {
        Some(model) => format!("{} · {model}", r.agent),
        None => r.agent.clone(),
    });
    let member = run.and_then(|r| r.member(role));
    let status = member.filter(|m| !m.closed).and_then(|m| m.status.clone());
    let working = status.as_deref() == Some("working");
    let holder = run.and_then(|r| r.holder.as_deref()) == Some(role);
    let pane = member.filter(|m| !m.closed).and_then(|m| m.pane);
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
        .when_some(pane, |d, pane| {
            d.cursor_pointer().hover(|s| s.bg(colours::hover())).on_click(move |_: &ClickEvent, _, cx| {
                Store::get(cx).update(cx, |s, cx| {
                    s.focus("pane", pane);
                    s.close_sheet(cx);
                });
            })
        })
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(div().size(px(7.)).rounded_full().bg(colour))
                .child(div().font_weight(FontWeight::SEMIBOLD).text_size(px(12.)).text_color(colour).child(name))
                .when(run.is_some(), |d| d.child(ui::badge(status.as_deref()))),
        )
        .children(agent.map(|a| ui::dim(a).pl(px(13.))))
}
