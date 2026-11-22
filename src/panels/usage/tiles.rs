use std::cmp::Ordering;

use gpui::{AnimationExt as _, Div, FontWeight, IntoElement, ParentElement as _, Styled as _, div, px};

use super::data::{self, Stats, WINDOWS};
use super::series::{self, Versus};
use super::widgets::{card, heading, rise};
use crate::theme::colours;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Shape {
    Count,
    Tokens,
    Time,
}

impl Shape {
    pub(super) fn show(self, n: f64) -> String {
        match self {
            Shape::Count => data::count(whole(n)),
            Shape::Tokens => data::tokens(whole(n)),
            Shape::Time => data::duration(n),
        }
    }

    fn daily(self, n: f64) -> String {
        match self {
            Shape::Count => series::decimal(n),
            Shape::Tokens | Shape::Time => self.show(n),
        }
    }
}

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "a counter shown whole, clamped at zero")]
fn whole(n: f64) -> u64 {
    n.max(0.).round() as u64
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Figure {
    pub label: &'static str,
    pub shape: Shape,
    pub values: [f64; 3],
    pub suffix: &'static str,
}

impl Figure {
    pub(super) fn sub(&self, window: usize, days: u64) -> Option<Versus> {
        let daily = |over: u64| Versus {
            words: format!("{}{} a day", self.shape.daily(series::per_day(self.values[window], over)), self.suffix),
            way: Ordering::Equal,
        };
        match window {
            0 if !self.suffix.is_empty() => None,
            0 => series::versus(self.values[0], self.values[1]),
            1 => Some(daily(7)),
            _ => Some(daily(days)),
        }
    }
}

pub(super) fn figures(stats: &Stats) -> Vec<Figure> {
    let windows = WINDOWS.map(|(key, _)| stats.window(key));
    let each = |f: &dyn Fn(&data::Window) -> f64| [f(&windows[0]), f(&windows[1]), f(&windows[2])];
    let mut figures = vec![
        Figure {
            label: "agents started",
            shape: Shape::Count,
            values: each(&|w| series::real(w.counts.agents)),
            suffix: "",
        },
        Figure { label: "prompts", shape: Shape::Count, values: each(&|w| series::real(w.counts.prompts)), suffix: "" },
        Figure {
            label: "tokens out",
            shape: Shape::Tokens,
            values: each(&|w| series::real(w.counts.tokens.out)),
            suffix: "",
        },
    ];
    figures.push(match &stats.merged_pull_requests {
        Some(m) => Figure {
            label: "merged PRs",
            shape: Shape::Count,
            values: [m.day, m.week, m.total].map(series::real),
            suffix: if m.complete { "" } else { "+" },
        },
        None => Figure {
            label: "waited for you",
            shape: Shape::Time,
            values: each(&|w| w.counts.waited_seconds),
            suffix: "",
        },
    });
    figures
}

pub(super) fn tiles(stats: &Stats, window: usize, days: u64) -> impl IntoElement + use<> {
    let shown: Vec<(Figure, Option<Versus>)> = figures(stats)
        .into_iter()
        .map(|f| {
            let sub = f.sub(window, days);
            (f, sub)
        })
        .collect();
    div().flex().flex_wrap().gap(px(12.)).with_animation(("usage-tiles", window), rise(), move |row, t| {
        row.children(shown.iter().map(|(figure, sub)| tile(figure, window, sub.as_ref(), t)))
    })
}

fn tile(figure: &Figure, window: usize, sub: Option<&Versus>, t: f32) -> Div {
    let value = figure.shape.show(figure.values[window] * f64::from(t));
    let sub = sub.map(|v| {
        let colour = if v.way == Ordering::Greater { colours::done() } else { colours::muted() };
        div().text_size(px(12.)).text_color(colour).child(v.words.clone())
    });
    card()
        .flex_1()
        .min_w(px(150.))
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(heading(figure.label))
        .child(
            div()
                .text_size(px(30.))
                .line_height(px(34.))
                .font_weight(FontWeight::BOLD)
                .child(format!("{value}{}", figure.suffix)),
        )
        .children(sub)
}

#[cfg(test)]
mod tests;
