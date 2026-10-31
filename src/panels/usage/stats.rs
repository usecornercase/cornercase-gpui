use gpui::{
    Div, FontWeight, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, Rgba, SharedString, Styled as _,
    div, prelude::FluentBuilder as _, px,
};

use super::data::{self, Stats, WEEKS, WINDOWS};
use super::widgets::{card, heading, plural};
use crate::theme::{self, colours, radius};
use crate::ui;

const CELL: f32 = 13.;
const GAP: f32 = 3.;

pub(super) fn hero(stats: &Stats, days: u64) -> impl IntoElement + use<> {
    let most = stats.window("24h").most_at_once;
    let side = div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .w(px(180.))
        .flex_none()
        .child(stat_pill("most at once", most.to_string()))
        .child(stat_pill("counting for", plural(days, "day", "days")));
    let Some((factor, agent, busy)) = stats.parallel() else {
        return idle_hero().child(side);
    };
    let busy_share = if agent > 0. { data::ratio(busy, agent) } else { 0. };
    card()
        .flex()
        .items_center()
        .gap(px(24.))
        .child(parallel_factor(factor))
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(time_bar("agent work", data::duration(agent), 1., colours::accent()))
                .child(time_bar("wall time busy", data::duration(busy), busy_share, colours::muted())),
        )
        .child(side)
}

fn idle_hero() -> Div {
    card().flex().items_center().gap(px(20.)).child(
        div()
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(div().text_size(px(18.)).text_color(colours::muted()).child("No agent worked in the last 24 h"))
            .child(ui::dim("the parallel factor shows once agents have been busy for a minute")),
    )
}

fn parallel_factor(factor: f64) -> Div {
    div()
        .flex()
        .flex_col()
        .flex_none()
        .child(
            div()
                .text_size(px(40.))
                .line_height(px(44.))
                .font_weight(FontWeight::BOLD)
                .text_color(colours::accent())
                .child(format!("{factor:.1}×")),
        )
        .child(div().text_color(colours::muted()).child("in parallel · last 24 h"))
}

fn time_bar(label: &'static str, value: String, fraction: f32, colour: Rgba) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(
            div()
                .flex()
                .text_size(px(12.))
                .child(div().text_color(colours::muted()).child(label))
                .child(div().flex_1())
                .child(value),
        )
        .child(ui::bar(fraction, colour, 8.))
}

fn stat_pill(label: &'static str, value: String) -> Div {
    div()
        .flex()
        .items_center()
        .px(px(10.))
        .py(px(6.))
        .rounded(px(radius::CONTROL))
        .bg(colours::panel())
        .text_size(px(12.))
        .child(div().text_color(colours::muted()).child(label))
        .child(div().flex_1())
        .child(div().font_weight(FontWeight::SEMIBOLD).child(value))
}

fn column(text: SharedString, colour: Rgba) -> Div {
    div().w(px(78.)).flex().justify_end().text_color(colour).child(text)
}

pub(super) fn table(stats: &Stats) -> impl IntoElement + use<> {
    let header = div()
        .flex()
        .px(px(8.))
        .pb(px(6.))
        .child(div().flex_1())
        .children(WINDOWS.iter().map(|(_, label)| column((*label).into(), colours::faint()).text_size(px(11.))));
    let rows = stats.rows().into_iter().enumerate().map(|(i, (label, values))| {
        div()
            .id(("usage-row", i))
            .flex()
            .px(px(10.))
            .py(px(6.))
            .rounded(px(radius::CHIP))
            .when(i % 2 == 1, |d| d.bg(colours::panel()))
            .hover(|s| s.bg(colours::hover()))
            .child(div().flex_1().text_color(colours::muted()).child(label))
            .children(values.into_iter().map(|v| column(v.into(), colours::text())))
    });
    card().flex().flex_col().child(header).children(rows).children(merged_errors(stats))
}

fn merged_errors(stats: &Stats) -> Option<Div> {
    let errors: Vec<String> = stats
        .merged_pull_requests
        .iter()
        .flat_map(|m| &m.accounts)
        .filter_map(|a| {
            let who = a.login.as_deref().map_or_else(|| a.host.clone(), |login| format!("{login} on {}", a.host));
            a.error.as_ref().map(|e| format!("{} ({who}): {e}", a.provider))
        })
        .collect();
    (!errors.is_empty()).then(|| {
        div()
            .pt(px(8.))
            .px(px(8.))
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(ui::dim("merged PRs marked + are a lower bound, some accounts could not be asked:"))
            .children(errors.into_iter().map(ui::dim))
    })
}

fn level_colour(level: u8) -> Hsla {
    match level {
        0 => colours::border().into(),
        1 => theme::hsla(theme::palette(24)),
        2 => theme::hsla(theme::palette(26)),
        3 => theme::hsla(theme::palette(33)),
        _ => theme::hsla(theme::palette(75)),
    }
}

pub(super) fn heatmap(stats: &Stats, days: u64) -> impl IntoElement + use<> {
    let levels = stats.levels();
    let level = |i: usize| levels.get(i).copied().unwrap_or(0);
    let columns =
        (0..WEEKS).map(|c| {
            let week = WEEKS - 1 - c;
            div().flex().flex_col().gap(px(GAP)).children(
                (0..7).map(|r| div().size(px(CELL)).rounded(px(2.)).bg(level_colour(level(week * 7 + 6 - r)))),
            )
        });
    card()
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(10.))
        .child(
            div()
                .flex()
                .items_center()
                .child(heading("agent time, last 26 weeks"))
                .child(div().flex_1())
                .child(legend()),
        )
        .child(div().flex().gap(px(GAP)).children(columns))
        .child(ui::dim(format!("one block per 24 h, the newest bottom right · {}", data::counting(days))))
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
