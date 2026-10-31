use gpui::{AnyElement, Div, FontWeight, IntoElement as _, ParentElement as _, Styled as _, div, px};

use super::{glass, motion};
use crate::theme::colours;

const GRID: f32 = 12.;

fn cell(unit: f32, (x, y): (f32, f32), (w, h): (f32, f32)) -> Div {
    div().absolute().left(px(x * unit)).top(px(y * unit)).w(px(w * unit)).h(px(h * unit))
}

fn cursor(unit: f32, blinking: bool) -> AnyElement {
    let ink = cell(unit, (6., 6.), (2., 5.)).bg(colours::text());
    if blinking { motion::pulse(ink, "mark-cursor").into_any_element() } else { ink.into_any_element() }
}

pub(crate) fn mark(unit: f32, blinking: bool) -> Div {
    let corner =
        |at, size| cell(unit, at, size).bg(colours::brand()).shadow(glass::glow(colours::brand(), 0.55, unit * 3.));
    div()
        .relative()
        .flex_none()
        .size(px(GRID * unit))
        .child(corner((3., 0.), (9., 3.)))
        .child(corner((0., 3.), (3., 9.)))
        .child(cursor(unit, blinking))
}

pub(crate) fn wordmark(size: f32) -> Div {
    div().text_size(px(size)).font_weight(FontWeight::BOLD).text_color(colours::text()).child("cornercase")
}
