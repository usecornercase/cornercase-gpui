use gpui::{
    Div, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, Stateful, StatefulInteractiveElement as _,
    Styled as _, div, px,
};

use super::data::{self, Stats, WEEKS};
use super::series;
use super::widgets::{card, heading, plural};
use crate::notify;
use crate::theme::{self, colours};
use crate::ui;

const CELL: f32 = 13.;
const GAP: f32 = 3.;
const MONTH_ROOM: usize = 3;

fn level_colour(level: u8) -> Hsla {
    match level {
        0 => colours::border().into(),
        1 => theme::hsla(theme::palette(24)),
        2 => theme::hsla(theme::palette(26)),
        3 => theme::hsla(theme::palette(33)),
        _ => theme::hsla(theme::palette(75)),
    }
}

pub(super) fn heatmap(stats: &Stats, days: u64, now: i64) -> impl IntoElement + use<> {
    let levels = stats.levels();
    let columns = (0..WEEKS).map(|c| {
        let week = WEEKS - 1 - c;
        div().flex().flex_col().gap(px(GAP)).children((0..7).map(|r| {
            let ago = week * 7 + 6 - r;
            block(ago, levels.get(ago).copied().unwrap_or(0), stats.seconds_ago(ago), now)
        }))
    });
    card()
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(
            div()
                .flex()
                .items_center()
                .child(heading("agent time, last 26 weeks"))
                .child(div().flex_1())
                .child(legend()),
        )
        .child(months(now))
        .child(div().flex().gap(px(GAP)).children(columns))
        .child(facts(stats, now))
        .child(ui::dim(format!("one block per 24 h, the newest bottom right · {}", data::counting(days))))
}

fn block(ago: usize, level: u8, seconds: f64, now: i64) -> Stateful<Div> {
    let edge = if ago == 0 { colours::soft().into() } else { level_colour(level) };
    div()
        .id(("usage-day", ago))
        .size(px(CELL))
        .rounded(px(3.))
        .border_1()
        .border_color(edge)
        .bg(level_colour(level))
        .hover(|s| s.border_color(colours::text()))
        .tooltip(notify::tip(series::day_tip(ago, seconds, now)))
}

fn months(now: i64) -> Div {
    let tops: Vec<u32> = (0..WEEKS).map(|c| series::month_of((WEEKS - 1 - c) * 7 + 6, now)).collect();
    let marks = series::month_marks(&tops, MONTH_ROOM);
    let label = |c: usize| marks.iter().find(|(at, _)| *at == c).map(|(_, name)| *name);
    div().flex().gap(px(GAP)).h(px(12.)).children((0..WEEKS).map(|c| {
        div()
            .w(px(CELL))
            .flex_none()
            .whitespace_nowrap()
            .text_size(px(10.))
            .text_color(colours::muted())
            .children(label(c))
    }))
}

fn facts(stats: &Stats, now: i64) -> Div {
    let streak = stats.streak() as u64;
    let best = stats
        .best()
        .map(|(ago, seconds)| fact("best", format!("{} · {}", series::day_name(ago, now), data::duration(seconds))));
    div()
        .flex()
        .flex_wrap()
        .gap(px(14.))
        .text_size(px(12.))
        .child(fact("streak", plural(streak, "day", "days")))
        .children(best)
        .child(fact("active", format!("{} of {}", stats.active(), series::SHOWN)))
}

fn fact(label: &'static str, value: String) -> Div {
    div().flex().gap(px(5.)).child(div().text_color(colours::muted()).child(label)).child(value)
}

fn legend() -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(3.))
        .text_size(px(11.))
        .text_color(colours::faint())
        .child(div().pr(px(3.)).child("less"))
        .children((1..=4).map(|l| div().size(px(10.)).rounded(px(2.)).bg(level_colour(l))))
        .child(div().pl(px(3.)).child("more"))
}
