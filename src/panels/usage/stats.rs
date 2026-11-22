use gpui::{
    AnimationExt as _, AnyElement, Div, FontWeight, HighlightStyle, InteractiveElement as _, IntoElement,
    ParentElement as _, Rgba, SharedString, Styled as _, StyledText, div, linear_color_stop, linear_gradient,
    prelude::FluentBuilder as _, px,
};

use super::data::{self, Stats, WINDOWS};
use super::gauge::gauge;
use super::series;
use super::widgets::{card, heading, plural, rise};
use crate::theme::{colours, radius};
use crate::ui;
use crate::ui::glass::tint;

const EYEBROWS: [&str; 3] = ["last 24 hours", "last 7 days", "since counting began"];
const SUBS: [&str; 3] = ["in parallel · last 24 h", "in parallel · last 7 days", "in parallel · all time"];

#[derive(Clone, Copy)]
struct Hero {
    factor: f64,
    agent: f64,
    busy: f64,
    most: u64,
    peak: u64,
    window: usize,
    summed: bool,
}

pub(super) fn hero(stats: &Stats, window: usize, summed: bool) -> AnyElement {
    let key = WINDOWS[window].0;
    let Some((factor, agent, busy)) = stats.parallel(key) else {
        return idle_hero(series::idle_words(window, stats.window(key).counts.agent_seconds)).into_any_element();
    };
    let most = stats.window(key).most_at_once;
    let peak = stats.window("total").most_at_once.max(most).max(1);
    let hero = Hero { factor, agent, busy, most, peak, window, summed };
    hero_card()
        .with_animation(("usage-hero", window), rise(), move |card, t| {
            card.child(gauge(hero.factor, t, SUBS[hero.window])).child(hero.story(t))
        })
        .into_any_element()
}

fn hero_card() -> Div {
    let wash = linear_gradient(
        135.,
        linear_color_stop(tint(colours::brand(), 0.22), 0.),
        linear_color_stop(colours::terminal(), 0.65),
    );
    card().flex().flex_wrap().items_center().gap(px(28.)).bg(wash)
}

impl Hero {
    fn story(self, t: f32) -> Div {
        div()
            .flex_1()
            .min_w(px(260.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(heading(EYEBROWS[self.window]))
            .child(headline(self.agent, self.busy, self.summed))
            .child(div().flex().child(gained(self.agent - self.busy)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(9.))
                    .pt(px(4.))
                    .child(lane("agent work", data::duration(self.agent), t, colours::brand()))
                    .child(lane(
                        series::busy_words(self.summed).1,
                        data::duration(self.busy),
                        data::ratio(self.busy, self.agent) * t,
                        colours::muted(),
                    ))
                    .child(lane(
                        "most at once",
                        plural(self.most, "agent", "agents"),
                        data::ratio(series::real(self.most), series::real(self.peak)) * t,
                        colours::accent(),
                    )),
            )
    }
}

fn headline(agent: f64, busy: f64, summed: bool) -> Div {
    let (worked, clock) = (data::duration(agent), data::duration(busy));
    let first = "Your agents did ".len();
    let second = first + worked.len() + " of work in ".len();
    let text = format!("Your agents did {worked} of work in {clock} {}", series::busy_words(summed).0);
    let lit = HighlightStyle { color: Some(colours::accent().into()), ..HighlightStyle::default() };
    let styled = StyledText::new(text)
        .with_highlights([(first..first + worked.len(), lit), (second..second + clock.len(), lit)]);
    div().text_size(px(20.)).line_height(px(26.)).font_weight(FontWeight::SEMIBOLD).child(styled)
}

fn gained(seconds: f64) -> Div {
    div()
        .px(px(10.))
        .py(px(4.))
        .rounded_full()
        .bg(tint(colours::done(), 0.14))
        .text_color(colours::done())
        .text_size(px(12.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(format!("+{} over one agent at a time", data::duration(seconds)))
}

fn lane(label: &'static str, value: String, fraction: f32, colour: Rgba) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .text_size(px(12.))
        .child(div().w(px(104.)).flex_none().text_color(colours::muted()).child(label))
        .child(div().flex_1().child(ui::bar(fraction, colour, 9.)))
        .child(div().w(px(76.)).flex_none().flex().justify_end().font_weight(FontWeight::SEMIBOLD).child(value))
}

fn idle_hero(words: &'static str) -> Div {
    card().flex().items_center().gap(px(20.)).child(
        div()
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(div().text_size(px(18.)).text_color(colours::muted()).child(words))
            .child(ui::dim("the parallel factor shows once agents have been busy for a minute")),
    )
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
            .rounded(px(radius::chip()))
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
