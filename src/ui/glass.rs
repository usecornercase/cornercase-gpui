use gpui::{
    Background, BoxShadow, Div, Hsla, ParentElement, Rgba, Styled, div, linear_color_stop, linear_gradient, point, px,
    rgba,
};

use crate::theme::{colours, radius};

const WASH: f32 = 180.;

pub(crate) fn tint(colour: Rgba, alpha: f32) -> Rgba {
    Rgba { a: alpha, ..colour }
}

fn shadow(colour: Hsla, y: f32, blur: f32) -> BoxShadow {
    BoxShadow { color: colour, offset: point(px(0.), px(y)), blur_radius: px(blur), spread_radius: px(0.) }
}

pub(crate) fn glow(colour: Rgba, alpha: f32, blur: f32) -> Vec<BoxShadow> {
    vec![shadow(tint(colour, alpha).into(), 0., blur)]
}

pub(crate) fn ring(colour: Rgba) -> Vec<BoxShadow> {
    vec![BoxShadow { spread_radius: px(3.), ..shadow(tint(colour, 0.32).into(), 0., 0.) }]
}

pub(crate) fn lift() -> Vec<BoxShadow> {
    let dark = |a: f32| Hsla { h: 0., s: 0., l: 0., a };
    vec![shadow(dark(0.5), 24., 60.), shadow(dark(0.3), 4., 12.)]
}

fn fade(angle: f32, colour: Rgba, from: f32, to: f32) -> Background {
    linear_gradient(angle, linear_color_stop(tint(colour, from), 0.), linear_color_stop(tint(colour, to), 1.))
}

pub(crate) fn cobalt() -> Background {
    linear_gradient(180., linear_color_stop(colours::brand_light(), 0.), linear_color_stop(colours::brand(), 1.))
}

fn sheen() -> Background {
    linear_gradient(180., linear_color_stop(rgba(0x1c_1f_24_e0), 0.), linear_color_stop(rgba(0x10_12_15_e8), 1.))
}

fn highlight(corner: f32) -> Div {
    div().absolute().top_0().left(px(corner)).right(px(corner)).h(px(1.)).bg(fade(90., colours::text(), 0.18, 0.02))
}

pub(crate) fn card(corner: f32) -> Div {
    div()
        .relative()
        .rounded(px(corner))
        .bg(sheen())
        .border_1()
        .border_color(colours::border())
        .shadow(lift())
        .child(highlight(corner))
}

pub(crate) fn dragged() -> Div {
    let halo = glow(colours::brand(), 0.35, 14.);
    div()
        .rounded(px(radius::CONTROL))
        .bg(colours::raised())
        .border_1()
        .border_color(colours::accent())
        .text_color(colours::text())
        .shadow(halo)
}

pub(crate) fn popover() -> Div {
    div().rounded(px(radius::CONTROL)).bg(colours::raised()).border_1().border_color(colours::edge()).shadow(lift())
}

pub(crate) fn wash() -> Div {
    div().absolute().top_0().left_0().right_0().h(px(WASH)).bg(fade(180., colours::brand(), 0.12, 0.))
}

pub(crate) fn lit<E: Styled>(row: E) -> E {
    row.bg(tint(colours::brand(), 0.26))
}
