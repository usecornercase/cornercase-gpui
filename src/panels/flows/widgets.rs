use gpui::{App, ClickEvent, Div, ParentElement as _, Rgba, SharedString, Styled as _, Window, div, px};

use super::model::FlowInfo;
use crate::store::Store;
use crate::theme::colours;
use crate::ui;

pub(super) fn tag(text: &'static str) -> Div {
    tag_owned(text.to_string())
}

pub(super) fn tag_owned(text: String) -> Div {
    div()
        .flex_none()
        .px(px(5.))
        .rounded(px(4.))
        .bg(colours::selected())
        .text_size(px(10.))
        .text_color(colours::muted())
        .child(text)
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
    move |_: &ClickEvent, _, cx| cx.open_with_system(std::path::Path::new(&path))
}

pub(super) fn error_small(text: impl Into<SharedString>) -> Div {
    div().text_size(px(11.)).text_color(colours::failed()).child(text.into())
}

pub(super) fn error_text(text: impl Into<SharedString>) -> Div {
    div().text_size(px(12.)).text_color(colours::failed()).child(text.into())
}

pub(super) fn role_label(colour: Rgba, name: String) -> Div {
    div()
        .w(px(150.))
        .flex_none()
        .flex()
        .items_center()
        .gap(px(6.))
        .child(dot(colour))
        .child(div().truncate().text_color(colour).child(name))
}

pub(super) fn dot(colour: Rgba) -> Div {
    div().size(px(7.)).rounded_full().bg(colour)
}

pub(super) fn text_box() -> Div {
    div()
        .p(px(10.))
        .rounded(px(6.))
        .bg(colours::terminal())
        .border_1()
        .border_color(colours::border())
        .text_size(px(12.))
}

pub(super) fn second_line(flow: &FlowInfo) -> Div {
    match flow.problems.first() {
        Some(problem) => error_small(problem.clone()).truncate(),
        None => ui::dim(flow.description.clone()).truncate(),
    }
}

pub(super) fn agents_label(flow: &FlowInfo) -> String {
    format!("{} agents", flow.agents())
}
