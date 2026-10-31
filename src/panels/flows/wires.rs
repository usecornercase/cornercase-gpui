use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    Background, Bounds, Canvas, Path, PathBuilder, Pixels, Point, Rgba, Styled as _, Window, canvas, fill,
    linear_color_stop, linear_gradient, point, px, size,
};

use super::graph::{Curve, HEAD, Spot, Way};
use crate::ui::glass::tint;

const GRID: f32 = 18.;
const DOT: f32 = 1.5;
const DASH: f32 = 5.;
const HALO: f32 = 7.;
const SPARK: f32 = 5.;

#[derive(Clone, Copy, Debug)]
pub(super) struct Ink {
    pub start: Rgba,
    pub end: Rgba,
    pub width: f32,
    pub glow: f32,
    pub dashed: bool,
}

impl Ink {
    pub(super) fn plain(start: Rgba, end: Rgba, width: f32) -> Self {
        Self { start, end, width, glow: 0., dashed: false }
    }
}

#[derive(Clone, Debug)]
pub(super) struct Line {
    pub curves: Vec<Curve>,
    pub tip: Option<(Spot, Way)>,
    pub ink: Ink,
}

pub(super) fn lines(lines: Vec<Line>) -> Canvas<()> {
    canvas(
        |_, _, _| {},
        move |bounds, (), window, _| {
            for line in &lines {
                paint(window, bounds.origin, line);
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

pub(super) fn backdrop(colour: Rgba, room: Rc<Cell<f32>>) -> Canvas<()> {
    canvas(
        move |bounds, window, _| {
            let width = f32::from(bounds.size.width);
            if (room.get() - width).abs() > 0.5 {
                room.set(width);
                window.request_animation_frame();
            }
        },
        move |bounds, (), window, _| paint_dots(window, bounds, colour),
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

pub(super) fn spark(at: Spot, colour: Rgba, shown: f32) -> Canvas<()> {
    canvas(
        |_, _, _| {},
        move |bounds, (), window, _| {
            let centre = place(bounds.origin, at);
            for (radius, alpha) in [(SPARK * 2.6, 0.12), (SPARK * 1.7, 0.28), (SPARK, 1.)] {
                let corner = point(centre.x - px(radius), centre.y - px(radius));
                let disc =
                    fill(Bounds::new(corner, size(px(radius * 2.), px(radius * 2.))), tint(colour, alpha * shown));
                window.paint_quad(disc.corner_radii(px(radius)));
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

pub(super) fn diamond(at: Spot, half: f32, colour: Rgba) -> Canvas<()> {
    canvas(
        |_, _, _| {},
        move |bounds, (), window, _| {
            let c = place(bounds.origin, at);
            let mut builder = PathBuilder::fill();
            builder.move_to(point(c.x, c.y - px(half)));
            builder.line_to(point(c.x + px(half), c.y));
            builder.line_to(point(c.x, c.y + px(half)));
            builder.line_to(point(c.x - px(half), c.y));
            builder.close();
            if let Ok(path) = builder.build() {
                window.paint_path(path, colour);
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

fn place(origin: Point<Pixels>, at: Spot) -> Point<Pixels> {
    point(origin.x + px(at.x), origin.y + px(at.y))
}

fn paint_dots(window: &mut Window, bounds: Bounds<Pixels>, colour: Rgba) {
    let (width, height) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    let mut y = GRID / 2.;
    while y < height {
        let mut x = GRID / 2.;
        while x < width {
            window.paint_quad(fill(Bounds::new(place(bounds.origin, Spot { x, y }), size(px(DOT), px(DOT))), colour));
            x += GRID;
        }
        y += GRID;
    }
}

fn paint(window: &mut Window, origin: Point<Pixels>, line: &Line) {
    let ink = line.ink;
    let rising = match (line.curves.first(), line.curves.last()) {
        (Some(first), Some(last)) => last.to.y < first.from.y,
        _ => return,
    };
    if ink.glow > 0. {
        let halo = shade(tint(ink.start, ink.glow), tint(ink.end, ink.glow), rising);
        if let Some(path) = stroke(origin, &line.curves, ink.width + HALO, false) {
            window.paint_path(path, halo);
        }
    }
    if let Some(path) = stroke(origin, &line.curves, ink.width, ink.dashed) {
        window.paint_path(path, shade(ink.start, ink.end, rising));
    }
    if let Some(head) = line.tip.and_then(|(tip, way)| head(origin, tip, way)) {
        window.paint_path(head, ink.end);
    }
}

fn shade(start: Rgba, end: Rgba, rising: bool) -> Background {
    let (from, to) = if rising { (end, start) } else { (start, end) };
    linear_gradient(180., linear_color_stop(from, 0.), linear_color_stop(to, 1.))
}

fn stroke(origin: Point<Pixels>, curves: &[Curve], width: f32, dashed: bool) -> Option<Path<Pixels>> {
    let first = curves.first()?;
    let mut builder = PathBuilder::stroke(px(width));
    if dashed {
        builder = builder.dash_array(&[px(DASH), px(DASH)]);
    }
    builder.move_to(place(origin, first.from));
    for curve in curves {
        builder.cubic_bezier_to(place(origin, curve.to), place(origin, curve.a), place(origin, curve.b));
    }
    builder.build().ok()
}

fn head(origin: Point<Pixels>, tip: Spot, way: Way) -> Option<Path<Pixels>> {
    let (half, long) = (HEAD * 0.62, HEAD + 1.);
    let [a, b] = match way {
        Way::Down => [Spot { x: tip.x - half, y: tip.y - long }, Spot { x: tip.x + half, y: tip.y - long }],
        Way::Left => [Spot { x: tip.x + long, y: tip.y - half }, Spot { x: tip.x + long, y: tip.y + half }],
        Way::Right => [Spot { x: tip.x - long, y: tip.y - half }, Spot { x: tip.x - long, y: tip.y + half }],
    };
    let mut builder = PathBuilder::fill();
    builder.move_to(place(origin, tip));
    builder.line_to(place(origin, a));
    builder.line_to(place(origin, b));
    builder.close();
    builder.build().ok()
}
