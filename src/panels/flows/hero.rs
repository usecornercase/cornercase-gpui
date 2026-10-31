use gpui::{
    Div, FontWeight, IntoElement, ParentElement as _, Rgba, SharedString, Styled as _, div, linear_color_stop,
    linear_gradient, prelude::FluentBuilder as _, px,
};

use super::cards::{avatar, ground};
use super::diagram::role_colour;
use super::graph::float;
use super::model::{FlowSpec, Progress, Run};
use super::widgets::{eyebrow, glyph, panel_box};
use crate::theme::{colours, radius};
use crate::ui::{self, Icon, glass, glass::tint};

const GLYPH: f32 = 50.;
const FACE: f32 = 26.;
const OVERLAP: f32 = 5.;

fn team_colours(flow: &FlowSpec) -> (Rgba, Rgba) {
    let first = flow.roles.first().map_or(colours::brand(), |r| role_colour(flow, &r.name));
    let last = flow.roles.last().map_or(colours::brand(), |r| role_colour(flow, &r.name));
    (first, last)
}

fn aurora(colour: Rgba, angle: f32, strength: f32) -> Div {
    div().absolute().top_0().left_0().size_full().rounded(px(radius::CARD)).bg(linear_gradient(
        angle,
        linear_color_stop(tint(colour, strength), 0.),
        linear_color_stop(tint(colour, 0.), 0.7),
    ))
}

fn shine() -> Div {
    div().absolute().top_0().left(px(radius::CARD)).right(px(radius::CARD)).h(px(1.)).bg(linear_gradient(
        90.,
        linear_color_stop(tint(colours::text(), 0.22), 0.),
        linear_color_stop(tint(colours::text(), 0.02), 1.),
    ))
}

pub(super) fn hero(flow: &FlowSpec) -> Div {
    let (first, last) = team_colours(flow);
    panel_box()
        .relative()
        .flex()
        .flex_col()
        .gap(px(14.))
        .p(px(20.))
        .child(aurora(colours::brand(), 180., 0.16))
        .child(aurora(first, 90., 0.18))
        .child(aurora(last, 270., 0.1))
        .child(shine())
}

pub(super) fn title(flow: &FlowSpec, run: Option<&Run>, name: SharedString, facts: Div) -> Div {
    div().relative().flex().items_center().gap(px(14.)).child(glyph(flow, run, GLYPH)).child(
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(5.))
            .child(div().text_size(px(22.)).font_weight(FontWeight::BOLD).child(ui::one_line(name)))
            .child(facts),
    )
}

pub(super) fn facts(parts: Vec<String>) -> Div {
    let dot = || div().text_color(colours::faint()).child("·");
    let mut line = div().flex().flex_wrap().items_center().gap(px(6.)).text_size(px(12.)).text_color(colours::muted());
    for (i, part) in parts.into_iter().enumerate() {
        line = line.when(i > 0, |d| d.child(dot())).child(part);
    }
    line
}

pub(super) fn team(flow: &FlowSpec) -> Div {
    let lead = div()
        .size(px(FACE))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(glass::cobalt())
        .border_2()
        .border_color(ground())
        .child(ui::icon(Icon::Waypoints, 13.).text_color(colours::on_brand()));
    let faces = flow
        .roles
        .iter()
        .map(|role| avatar(&role.name, role_colour(flow, &role.name), FACE).border_2().border_color(ground()));
    let step = FACE - OVERLAP;
    let placed = std::iter::once(lead)
        .chain(faces)
        .enumerate()
        .map(|(i, face)| face.absolute().top_0().left(px(float(i) * step)));
    div().relative().flex_none().w(px(FACE + float(flow.roles.len()) * step)).h(px(FACE)).children(placed)
}

pub(super) fn description(text: &str) -> Div {
    div()
        .relative()
        .max_w(px(640.))
        .text_size(px(13.))
        .line_height(px(19.))
        .text_color(colours::soft())
        .child(text.to_string())
}

pub(super) fn stages_bar(run: &Run) -> Div {
    div().relative().flex().gap(px(4.)).h(px(6.)).children((0..run.flow.stages.len()).map(|stage| segment(run, stage)))
}

fn segment(run: &Run, stage: usize) -> Div {
    let bar = div().flex_1().h_full().rounded_full();
    if run.closed() {
        return bar.bg(tint(colours::text(), 0.1));
    }
    match run.progress(stage) {
        Progress::Passed => bar.bg(tint(colours::brand_light(), 0.75)),
        Progress::Active => bar.bg(colours::brand_light()).shadow(glass::glow(colours::brand(), 0.8, 10.)),
        Progress::Ahead => bar.bg(tint(colours::text(), 0.08)),
    }
}

pub(super) fn task(text: &str) -> impl IntoElement + use<> {
    panel_box()
        .flex()
        .gap(px(12.))
        .p(px(14.))
        .child(div().w(px(3.)).flex_none().rounded_full().bg(glass::cobalt()))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(eyebrow("Task"))
                .child(ui::clamped(8, 19.).text_size(px(13.)).child(text.to_string())),
        )
}
