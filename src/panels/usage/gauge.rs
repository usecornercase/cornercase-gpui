use std::f32::consts::PI;

use gpui::{
    Bounds, Div, FontWeight, Hsla, ParentElement as _, PathBuilder, Pixels, Point, Styled as _, Window, canvas, div,
    fill, point, px, size,
};

use super::data;
use super::series;
use crate::theme::colours;
use crate::ui::glass::tint;

const SIZE: f32 = 196.;
const WIDTH: f32 = 13.;
const GLOW: f32 = WIDTH * 2.4;
const START: f32 = 135.;
const SWEEP: f32 = 270.;
const SEGMENTS: usize = 135;
const TICK_INSET: f32 = 31.;

pub(super) fn gauge(factor: f64, shown: f32, sub: &'static str) -> Div {
    let top = series::gauge_top(factor);
    let filled = (data::ratio(factor, f64::from(top)) * shown).clamp(0., 1.);
    div()
        .relative()
        .size(px(SIZE))
        .flex_none()
        .child(canvas(|_, _, _| {}, move |bounds, (), window, _| paint(bounds, filled, window)).absolute().size_full())
        .children(series::gauge_ticks(top).into_iter().map(move |tick| tick_label(tick, top)))
        .child(centre(factor * f64::from(shown), sub))
}

fn centre(factor: f64, sub: &'static str) -> Div {
    div()
        .absolute()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .child(
            div()
                .text_size(px(44.))
                .line_height(px(48.))
                .font_weight(FontWeight::BOLD)
                .text_color(colours::text())
                .child(format!("{factor:.1}×")),
        )
        .child(div().text_size(px(11.)).text_color(colours::muted()).child(sub))
}

fn tick_label(tick: u32, top: u32) -> Div {
    let at = spot(data::ratio(f64::from(tick), f64::from(top)), SIZE / 2. - TICK_INSET);
    div()
        .absolute()
        .left(px(SIZE / 2. + at.x - 12.))
        .top(px(SIZE / 2. + at.y - 7.))
        .w(px(24.))
        .h(px(14.))
        .flex()
        .justify_center()
        .text_size(px(10.))
        .text_color(colours::faint())
        .child(format!("{tick}×"))
}

fn spot(along: f32, radius: f32) -> Point<f32> {
    let angle = (START + SWEEP * along) * PI / 180.;
    point(radius * angle.cos(), radius * angle.sin())
}

#[derive(Clone, Copy)]
struct Ring {
    bounds: Bounds<Pixels>,
    radius: f32,
}

impl Ring {
    fn at(self, along: f32) -> Point<Pixels> {
        let centre = self.bounds.center();
        let at = spot(along, self.radius);
        point(centre.x + px(at.x), centre.y + px(at.y))
    }

    fn arc(self, to: f32, width: f32) -> Option<gpui::Path<Pixels>> {
        let mut path = PathBuilder::stroke(px(width));
        for i in 0..=SEGMENTS {
            let at = self.at(to * series::along(i, SEGMENTS));
            if i == 0 { path.move_to(at) } else { path.line_to(at) }
        }
        path.build().ok()
    }

    fn cap(self, along: f32, width: f32, colour: impl Into<Hsla>, window: &mut Window) {
        let at = self.at(along);
        let corner = point(at.x - px(width / 2.), at.y - px(width / 2.));
        let dot = fill(Bounds::new(corner, size(px(width), px(width))), colour.into()).corner_radii(px(width / 2.));
        window.paint_quad(dot);
    }
}

fn paint(bounds: Bounds<Pixels>, filled: f32, window: &mut Window) {
    let ring = Ring { bounds, radius: SIZE / 2. - WIDTH };
    let track = colours::border();
    if let Some(path) = ring.arc(1., WIDTH) {
        window.paint_path(path, track);
    }
    ring.cap(0., WIDTH, track, window);
    ring.cap(1., WIDTH, track, window);
    if filled <= 0. {
        return;
    }
    let ink = colours::brand();
    if let Some(glow) = ring.arc(filled, GLOW) {
        window.paint_path(glow, tint(ink, 0.16));
    }
    if let Some(path) = ring.arc(filled, WIDTH) {
        window.paint_path(path, ink);
    }
    ring.cap(0., WIDTH, ink, window);
    ring.cap(filled, WIDTH, colours::brand_light(), window);
}
