use gpui::{
    App, Div, InteractiveElement as _, ParentElement as _, Rgba, Stateful, StatefulInteractiveElement as _,
    Styled as _, div, linear_color_stop, linear_gradient, px, relative,
};

use super::hsv::Hsv;
use super::{WIDTH, blank_drag};
use crate::theme::colour::{from_bytes, with_alpha};
use crate::theme::colours;
use crate::ui::{checker, track};

pub(super) const SQUARE: &str = "picker-square";
pub(super) const HUE: &str = "picker-hue";
pub(super) const ALPHA: &str = "picker-alpha";
const HEIGHT: f32 = 132.;
const STRIP: f32 = 12.;
const RING: f32 = 12.;

pub(super) struct Square {
    hsv: Hsv,
}

pub(super) struct Hue {
    hsv: Hsv,
}

pub(super) struct Alpha {
    colour: Rgba,
    alpha: f32,
}

fn clear() -> Rgba {
    with_alpha(from_bytes([0, 0, 0]), 0.)
}

fn layer() -> Div {
    div().absolute().inset_0()
}

impl Square {
    pub(super) fn new(hsv: Hsv) -> Self {
        Square { hsv }
    }

    pub(super) fn draw(&self) -> Stateful<Div> {
        let white = from_bytes([255, 255, 255]);
        let across = linear_gradient(90., linear_color_stop(white, 0.), linear_color_stop(self.hsv.pure(), 1.));
        let down = linear_gradient(180., linear_color_stop(clear(), 0.), linear_color_stop(from_bytes([0, 0, 0]), 1.));
        let ring = div()
            .absolute()
            .left(relative(self.hsv.s))
            .top(relative(1. - self.hsv.v))
            .ml(px(-RING / 2.))
            .mt(px(-RING / 2.))
            .size(px(RING))
            .rounded_full()
            .border_2()
            .border_color(white)
            .shadow(crate::ui::glass::glow(from_bytes([0, 0, 0]), 0.6, 3.));
        div()
            .id(SQUARE)
            .relative()
            .w(px(WIDTH - 20.))
            .h(px(HEIGHT))
            .rounded(px(6.))
            .overflow_hidden()
            .cursor_crosshair()
            .child(track::remember(&SQUARE.into()))
            .child(layer().bg(across))
            .child(layer().bg(down))
            .child(ring)
            .on_drag(Square { hsv: self.hsv }, |_, _, _, cx: &mut App| blank_drag(cx))
    }
}

impl Hue {
    pub(super) fn new(hsv: Hsv) -> Self {
        Hue { hsv }
    }

    pub(super) fn draw(&self) -> Stateful<Div> {
        let stops: Vec<Rgba> =
            (0..=6u8).map(|i| Hsv { h: (f32::from(i) / 6.).min(0.9999), s: 1., v: 1. }.pure()).collect();
        let segments = stops.windows(2).map(|pair| {
            div().flex_1().h_full().bg(linear_gradient(
                90.,
                linear_color_stop(pair[0], 0.),
                linear_color_stop(pair[1], 1.),
            ))
        });
        let mark = marker(self.hsv.h);
        div()
            .id(HUE)
            .relative()
            .w(px(WIDTH - 20.))
            .h(px(STRIP))
            .rounded_full()
            .overflow_hidden()
            .cursor_pointer()
            .child(track::remember(&HUE.into()))
            .child(div().absolute().inset_0().flex().children(segments))
            .child(mark)
            .on_drag(Hue { hsv: self.hsv }, |_, _, _, cx: &mut App| blank_drag(cx))
    }
}

impl Alpha {
    pub(super) fn new(colour: Rgba, alpha: f32) -> Self {
        Alpha { colour, alpha }
    }

    pub(super) fn draw(&self) -> Stateful<Div> {
        let fade = linear_gradient(
            90.,
            linear_color_stop(with_alpha(self.colour, 0.), 0.),
            linear_color_stop(with_alpha(self.colour, 1.), 1.),
        );
        div()
            .id(ALPHA)
            .relative()
            .w(px(WIDTH - 20.))
            .h(px(STRIP))
            .rounded_full()
            .overflow_hidden()
            .cursor_pointer()
            .child(track::remember(&ALPHA.into()))
            .child(checker(WIDTH - 20., STRIP, 6.))
            .child(layer().bg(fade))
            .child(marker(self.alpha))
            .on_drag(Alpha { colour: self.colour, alpha: self.alpha }, |_, _, _, cx: &mut App| blank_drag(cx))
    }
}

fn marker(at: f32) -> Div {
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .left(relative(at))
        .ml(px(-1.5))
        .w(px(3.))
        .bg(colours::white())
        .border_1()
        .border_color(from_bytes([0, 0, 0]))
        .rounded_full()
}
