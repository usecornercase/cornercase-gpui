use gpui::{
    Div, ElementId, InteractiveElement as _, ParentElement as _, Rgba, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, div, px,
};

use super::{Icon, glass, icon};
use crate::notify::tip;
use crate::theme::{colours, radius};

pub(crate) const ROW_ACTION: f32 = 26.;

fn chip(text: impl Into<SharedString>, colour: Rgba, border: Rgba) -> Div {
    div()
        .flex_none()
        .px(px(6.))
        .rounded(px(radius::CHIP))
        .text_size(px(10.5))
        .text_color(colour)
        .border_1()
        .border_color(border)
        .child(text.into())
}

pub(crate) fn accent_chip(text: impl Into<SharedString>) -> Div {
    chip(text, colours::accent(), glass::tint(colours::accent(), 0.45)).bg(glass::tint(colours::brand(), 0.14))
}

pub(crate) fn quiet_chip(text: impl Into<SharedString>) -> Div {
    chip(text, colours::muted(), colours::border())
}

pub(crate) fn tinted_chip(text: impl Into<SharedString>, colour: Rgba) -> Div {
    chip(text, colour, glass::tint(colour, 0.45)).bg(glass::tint(colour, 0.1))
}

pub(crate) fn dot(colour: Option<Rgba>) -> Div {
    let dot = div().size(px(7.)).flex_none().rounded_full();
    match colour {
        Some(colour) => dot.bg(colour),
        None => dot.border_1().border_color(colours::faint()),
    }
}

fn marked(glyph: Icon, colour: Rgba, text: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .items_start()
        .gap(px(6.))
        .text_size(px(12.))
        .text_color(colour)
        .child(div().pt(px(2.)).child(icon(glyph, 12.).text_color(colour)))
        .child(div().flex_1().min_w_0().child(text.into()))
}

pub(crate) fn error_line(text: impl Into<SharedString>) -> Div {
    marked(Icon::Alert, colours::failed(), text)
}

pub(crate) fn warning_line(text: impl Into<SharedString>) -> Div {
    marked(Icon::Alert, colours::waiting(), text).text_size(px(11.5))
}

pub(crate) fn done_line(text: impl Into<SharedString>) -> Div {
    marked(Icon::Check, colours::done(), text)
}

pub(crate) fn callout(colour: Rgba, title: impl Into<SharedString>, body: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .items_start()
        .gap(px(10.))
        .px(px(14.))
        .py(px(10.))
        .rounded(px(radius::CARD))
        .bg(glass::tint(colour, 0.08))
        .border_1()
        .border_color(glass::tint(colour, 0.3))
        .child(div().pt(px(1.)).child(icon(Icon::Alert, 15.).text_color(colour)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(div().text_size(px(13.)).text_color(colours::text()).child(title.into()))
                .child(div().text_size(px(12.)).text_color(colours::muted()).child(body.into())),
        )
}

pub(crate) fn row_action(
    id: impl Into<ElementId>,
    glyph: Icon,
    hint: impl Into<SharedString>,
    danger: bool,
) -> Stateful<Div> {
    let id: ElementId = id.into();
    let group = SharedString::from(format!("{id}"));
    let (ink, fill) = if danger {
        (colours::failed(), glass::tint(colours::failed(), 0.12))
    } else {
        (colours::text(), colours::hover())
    };
    div()
        .id(id)
        .group(group.clone())
        .size(px(ROW_ACTION))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::CHIP))
        .cursor_pointer()
        .tooltip(tip(hint))
        .hover(move |s| s.bg(fill))
        .child(icon(glyph, 14.).text_color(colours::faint()).group_hover(group, move |s| s.text_color(ink)))
}

pub(crate) fn resting_action(glyph: Icon) -> Div {
    div()
        .size(px(ROW_ACTION))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .opacity(0.35)
        .child(icon(glyph, 14.).text_color(colours::faint()))
}

pub(crate) fn field_action(id: impl Into<ElementId>, glyph: Icon, hint: &'static str, ink: Rgba) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(20.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::CHIP))
        .cursor_pointer()
        .tooltip(tip(hint))
        .hover(|s| s.bg(colours::selected()))
        .child(icon(glyph, 12.).text_color(ink))
}
