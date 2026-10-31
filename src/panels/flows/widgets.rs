use gpui::{
    App, ClickEvent, Div, ElementId, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, Rgba,
    SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, Window, div, linear_color_stop,
    linear_gradient, prelude::FluentBuilder as _, px, rgb,
};

use super::diagram::role_colour;
use super::graph::float;
use super::model::{FlowInfo, FlowSpec, Progress, Run};
use crate::cli;
use crate::notify::tip;
use crate::store::Store;
use crate::theme::{colours, radius};
use crate::ui::{self, Icon, glass, glass::tint};

pub(super) fn tag(text: &'static str) -> Div {
    let default = text == "default";
    let colour = if default { colours::accent() } else { colours::muted() };
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(3.))
        .px(px(6.))
        .rounded(px(radius::CHIP))
        .bg(if default { tint(colours::brand(), 0.2) } else { colours::selected() })
        .text_size(px(10.))
        .text_color(colour)
        .when(default, |d| d.child(ui::icon(Icon::StarFilled, 9.).text_color(colour)))
        .child(text)
}

pub(super) fn chip(glyph: Option<Icon>, text: impl Into<SharedString>, colour: Rgba) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(4.))
        .h(px(20.))
        .px(px(7.))
        .rounded(px(radius::CHIP))
        .bg(tint(colour, 0.1))
        .border_1()
        .border_color(tint(colour, 0.22))
        .text_size(px(11.))
        .text_color(colour)
        .whitespace_nowrap()
        .children(glyph.map(|g| ui::icon(g, 11.).text_color(colour)))
        .child(text.into())
}

pub(super) fn heading(glyph: Icon, text: &str) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(7.))
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(colours::muted())
        .child(ui::icon(glyph, 13.).text_color(colours::faint()))
        .child(text.to_uppercase())
}

pub(super) fn eyebrow(text: &str) -> Div {
    div().text_size(px(10.)).font_weight(FontWeight::SEMIBOLD).text_color(colours::faint()).child(text.to_uppercase())
}

pub(super) fn pill(text: impl Into<SharedString>, colour: Rgba, glowing: bool) -> Div {
    let light = div().size(px(6.)).rounded_full().bg(colour).when(glowing, |d| d.shadow(glass::glow(colour, 0.9, 6.)));
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(6.))
        .h(px(22.))
        .px(px(9.))
        .rounded_full()
        .bg(tint(colour, 0.12))
        .border_1()
        .border_color(tint(colour, 0.32))
        .text_size(px(11.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(colour)
        .child(light)
        .child(text.into())
}

pub(super) fn icon_action(
    id: impl Into<ElementId>,
    glyph: Icon,
    hint: impl Into<SharedString>,
    danger: bool,
) -> Stateful<Div> {
    let colour = if danger { colours::failed() } else { colours::soft() };
    let base = div()
        .id(id)
        .flex_none()
        .size(px(30.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::CONTROL))
        .border_1()
        .cursor_pointer()
        .tooltip(tip(hint))
        .child(ui::icon(glyph, 14.).text_color(colour));
    if danger {
        return base
            .bg(tint(colours::failed(), 0.08))
            .border_color(tint(colours::failed(), 0.3))
            .hover(|s| s.bg(tint(colours::failed(), 0.18)).border_color(tint(colours::failed(), 0.6)));
    }
    base.bg(colours::hover())
        .border_color(colours::border())
        .hover(|s| s.bg(colours::selected()).border_color(colours::edge()))
}

#[derive(Clone, Copy)]
struct Mini<'a> {
    flow: &'a FlowSpec,
    run: Option<&'a Run>,
    dot: f32,
    gap: f32,
}

pub(super) fn glyph(flow: &FlowSpec, run: Option<&Run>, size: f32) -> Div {
    let stages = flow.stages.len().max(1);
    let gap = (size / 14.).round().max(2.);
    let room = size * 0.7 - gap * float(stages - 1);
    let mini = Mini { flow, run, dot: (room / float(stages)).clamp(2., size / 6.).floor(), gap };
    div()
        .size(px(size))
        .flex_none()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(gap))
        .rounded(px((size * 0.27).round()))
        .bg(linear_gradient(160., linear_color_stop(rgb(0x25_29_31), 0.), linear_color_stop(rgb(0x12_14_18), 1.)))
        .border_1()
        .border_color(colours::edge())
        .children((0..flow.stages.len()).map(|stage| glyph_row(mini, stage)))
}

fn glyph_row(mini: Mini, stage: usize) -> Div {
    let progress = mini.run.map(|r| r.progress(stage));
    let roles = mini.flow.stages.get(stage).map(|s| s.roles.as_slice()).unwrap_or_default();
    div().flex().gap(px(mini.gap)).children(roles.iter().map(|role| {
        let colour = role_colour(mini.flow, role);
        let alpha = match progress {
            Some(Progress::Ahead) => 0.25,
            Some(Progress::Passed) => 0.7,
            _ => 1.,
        };
        div()
            .size(px(mini.dot))
            .rounded_full()
            .bg(tint(colour, alpha))
            .when(progress == Some(Progress::Active), |d| d.shadow(glass::glow(colour, 0.9, mini.dot * 2.)))
    }))
}

pub(super) fn file_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

pub(super) fn show_pane(pane: u64, cx: &mut App) {
    Store::get(cx).update(cx, |s, cx| {
        s.focus("pane", pane);
        s.close_sheet(cx);
    });
}

pub(super) fn open_path(path: String) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    move |_: &ClickEvent, _, cx| {
        if cli::host().is_some() {
            let path = path.clone();
            Store::get(cx).update(cx, |s, cx| s.copy_elsewhere(path, cx));
        } else {
            cx.open_with_system(std::path::Path::new(&path));
        }
    }
}

pub(super) fn error_small(text: impl IntoElement) -> Div {
    div().text_size(px(11.)).text_color(colours::failed()).child(text)
}

pub(super) fn error_text(text: impl Into<SharedString>) -> Div {
    div().text_size(px(12.)).text_color(colours::failed()).child(text.into())
}

pub(super) fn alert(text: impl Into<SharedString>, colour: Rgba) -> Div {
    div()
        .flex()
        .items_start()
        .gap(px(8.))
        .px(px(12.))
        .py(px(8.))
        .rounded(px(radius::CONTROL))
        .bg(tint(colour, 0.1))
        .border_1()
        .border_color(tint(colour, 0.3))
        .text_size(px(12.))
        .text_color(colour)
        .child(ui::icon(Icon::Alert, 14.).text_color(colour).mt(px(1.)))
        .child(div().flex_1().min_w_0().child(text.into()))
}

pub(super) fn panel_box() -> Div {
    div().rounded(px(radius::CARD)).bg(colours::panel()).border_1().border_color(colours::border())
}

pub(super) fn second_line(flow: &FlowInfo) -> Div {
    match flow.problems.first() {
        Some(problem) => error_small(ui::one_line(problem.clone())),
        None => ui::dim(ui::one_line(flow.description.clone())),
    }
}

pub(super) fn name_line(name: String) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .child(div().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child(ui::one_line(name)))
}

pub(super) fn flow_summary(flow: &FlowInfo) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(
            name_line(flow.name.clone())
                .children(flow.tags().into_iter().map(tag))
                .child(div().flex_1())
                .child(ui::dim(flow.agents().to_string()).flex_none())
                .child(ui::icon(Icon::Accounts, 11.).text_color(colours::faint())),
        )
        .child(second_line(flow))
}

pub(super) fn agents_label(flow: &FlowInfo) -> String {
    format!("{} agents", flow.agents())
}
