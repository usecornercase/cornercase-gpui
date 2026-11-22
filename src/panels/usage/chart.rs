use gpui::{
    Bounds, Div, InteractiveElement as _, IntoElement, ParentElement as _, PathBuilder, Pixels, Point, Rgba,
    StatefulInteractiveElement as _, Styled as _, Window, canvas, div, fill, linear_color_stop, linear_gradient, point,
    px, relative, size,
};

use super::data::{self, Stats};
use super::series::{self, SHOWN};
use super::widgets::{card, heading, swatch};
use crate::notify;
use crate::theme::colours;
use crate::ui::glass::tint;

const HEIGHT: f32 = 168.;
const GUTTER: f32 = 34.;
const SPAN: usize = 7;
const MONTH_ROOM: usize = 14;

#[derive(Clone)]
struct Plot {
    hours: Vec<f64>,
    mean: Vec<f64>,
    top: f64,
    step: f64,
    best: Option<usize>,
}

impl Plot {
    fn of(stats: &Stats) -> Self {
        let hours: Vec<f64> = stats.oldest_first().into_iter().map(|s| s / 3_600.).collect();
        let mean = series::trailing_mean(&hours, SPAN);
        let (top, step) = series::scale(hours.iter().copied().fold(0., f64::max));
        let best = stats.best().map(|(ago, _)| SHOWN - 1 - ago);
        Self { hours, mean, top, step, best }
    }

    fn at(&self, bounds: Bounds<Pixels>, i: usize, hours: f64) -> Point<Pixels> {
        let x = bounds.left() + bounds.size.width * series::along(i, SHOWN - 1);
        let y = bounds.bottom() - bounds.size.height * data::ratio(hours.min(self.top), self.top);
        point(x, y)
    }

    fn ticks(&self) -> Vec<f64> {
        (0..=8).map(|k: u32| self.step * f64::from(k)).take_while(|v| *v <= self.top + 1e-9).collect()
    }
}

pub(super) fn chart(stats: &Stats, now: i64) -> impl IntoElement + use<> {
    let plot = Plot::of(stats);
    card()
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(head(stats, now))
        .child(div().flex().child(axis(&plot)).child(area(&plot, stats, now)))
        .child(months(now))
}

fn head(stats: &Stats, now: i64) -> Div {
    let record = stats.best().map(|(ago, seconds)| {
        div().flex().items_center().gap(px(6.)).child(swatch(colours::waiting())).child(format!(
            "record {} · {}",
            data::duration(seconds),
            series::day_name(ago, now)
        ))
    });
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(14.))
        .child(heading("agent hours per day, last 26 weeks"))
        .child(div().flex_1())
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(div().w(px(14.)).h(px(2.)).rounded_full().bg(colours::soft()))
                .child("7-day average"),
        )
        .children(record)
        .text_size(px(11.))
        .text_color(colours::muted())
}

fn axis(plot: &Plot) -> Div {
    let label = |value: f64| {
        let from_top = HEIGHT * (1. - data::ratio(value, plot.top));
        div()
            .absolute()
            .right(px(8.))
            .top(px(from_top - 7.))
            .h(px(14.))
            .text_size(px(10.))
            .text_color(colours::faint())
            .child(format!("{value:.0}h"))
    };
    div().relative().w(px(GUTTER)).h(px(HEIGHT)).flex_none().children(plot.ticks().into_iter().map(label))
}

fn area(plot: &Plot, stats: &Stats, now: i64) -> Div {
    let drawn = plot.clone();
    let hits = (0..SHOWN).map(|i| {
        let ago = SHOWN - 1 - i;
        div()
            .id(("usage-chart-day", i))
            .flex_1()
            .h_full()
            .rounded(px(2.))
            .hover(|s| s.bg(tint(colours::text(), 0.08)))
            .tooltip(notify::tip(series::day_tip(ago, stats.seconds_ago(ago), now)))
    });
    div()
        .relative()
        .flex_1()
        .min_w_0()
        .h(px(HEIGHT))
        .child(canvas(|_, _, _| {}, move |bounds, (), window, _| paint(&drawn, bounds, window)).absolute().size_full())
        .child(div().absolute().size_full().flex().children(hits))
}

fn months(now: i64) -> Div {
    let shown: Vec<u32> = (0..SHOWN).rev().map(|ago| series::month_of(ago, now)).collect();
    let marks = series::month_marks(&shown, MONTH_ROOM).into_iter().map(|(i, name)| {
        div()
            .absolute()
            .left(relative(series::along(i, SHOWN - 1)))
            .whitespace_nowrap()
            .text_size(px(10.))
            .text_color(colours::muted())
            .child(name)
    });
    div().relative().h(px(14.)).ml(px(GUTTER)).children(marks)
}

fn paint(plot: &Plot, bounds: Bounds<Pixels>, window: &mut Window) {
    for value in plot.ticks() {
        let y = plot.at(bounds, 0, value).y;
        window
            .paint_quad(fill(Bounds::new(point(bounds.left(), y), size(bounds.size.width, px(1.))), colours::border()));
    }
    let wash = linear_gradient(
        180.,
        linear_color_stop(tint(colours::brand(), 0.5), 0.),
        linear_color_stop(tint(colours::brand(), 0.02), 1.),
    );
    if let Some(path) = filled(plot, bounds) {
        window.paint_path(path, wash);
    }
    if let Some(path) = line(plot, &plot.hours, bounds, 1.) {
        window.paint_path(path, tint(colours::accent(), 0.7));
    }
    if let Some(path) = line(plot, &plot.mean, bounds, 2.) {
        window.paint_path(path, colours::soft());
    }
    if let Some(i) = plot.best {
        dot(plot.at(bounds, i, plot.hours[i]), 8., colours::waiting(), window);
    }
    dot(plot.at(bounds, SHOWN - 1, plot.hours[SHOWN - 1]), 7., colours::text(), window);
}

fn filled(plot: &Plot, bounds: Bounds<Pixels>) -> Option<gpui::Path<Pixels>> {
    let mut path = PathBuilder::fill();
    path.move_to(point(bounds.left(), bounds.bottom()));
    for (i, hours) in plot.hours.iter().enumerate() {
        path.line_to(plot.at(bounds, i, *hours));
    }
    path.line_to(point(bounds.right(), bounds.bottom()));
    path.close();
    path.build().ok()
}

fn line(plot: &Plot, values: &[f64], bounds: Bounds<Pixels>, width: f32) -> Option<gpui::Path<Pixels>> {
    let mut path = PathBuilder::stroke(px(width));
    for (i, value) in values.iter().enumerate() {
        let at = plot.at(bounds, i, *value);
        if i == 0 { path.move_to(at) } else { path.line_to(at) }
    }
    path.build().ok()
}

fn dot(at: Point<Pixels>, width: f32, colour: Rgba, window: &mut Window) {
    let corner = point(at.x - px(width / 2.), at.y - px(width / 2.));
    window.paint_quad(fill(Bounds::new(corner, size(px(width), px(width))), colour).corner_radii(px(width / 2.)));
}
