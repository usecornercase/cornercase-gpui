use gpui::{
    Div, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, SharedString, Stateful, Styled as _, div, px,
    relative,
};

use super::data::{self, Counts};
use super::widgets::{bar, card, heading, swatch};
use crate::theme::{self, colours};
use crate::ui;

const COLUMNS: [&str; 7] = ["agent time", "started", "prompts", "turns", "waited", "tokens out", "tokens in"];

fn agent_colour(key: &str) -> Hsla {
    match key {
        "claude" => colours::waiting().into(),
        "codex" => theme::hsla(theme::ANSI[6]),
        "gemini" => theme::hsla(theme::ANSI[4]),
        "opencode" => theme::hsla(theme::ANSI[2]),
        "grok" => theme::hsla(theme::ANSI[7]),
        "cursor" => theme::hsla(theme::ANSI[5]),
        "copilot" => theme::hsla(theme::ANSI[1]),
        _ => colours::muted().into(),
    }
}

pub(super) fn by_agent(agents: &[(String, Counts)]) -> impl IntoElement + use<> {
    let most = agents.iter().map(|(_, c)| c.agent_seconds).fold(0., f64::max);
    div()
        .flex()
        .flex_col()
        .gap(px(14.))
        .child(card().flex().flex_col().gap(px(10.)).child(heading("share of agent time")).children(shares(agents)))
        .child(
            card()
                .flex()
                .flex_col()
                .child(header())
                .children(agents.iter().enumerate().map(|(i, (key, c))| agent_row(i, key, c, most))),
        )
}

fn shares(agents: &[(String, Counts)]) -> [Div; 2] {
    let total: f64 = agents.iter().map(|(_, c)| c.agent_seconds).sum();
    let share = div().flex().w_full().h(px(10.)).rounded(px(5.)).overflow_hidden().bg(colours::border()).children(
        agents.iter().filter(|(_, c)| c.agent_seconds > 0.).map(|(key, c)| {
            div().h_full().w(relative(data::ratio(c.agent_seconds, total.max(1e-9)))).bg(agent_colour(key))
        }),
    );
    let legend = div().flex().flex_wrap().gap(px(14.)).children(agents.iter().map(|(key, c)| {
        let percent = if total > 0. { c.agent_seconds / total * 100. } else { 0. };
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .text_size(px(12.))
            .child(swatch(agent_colour(key)))
            .child(data::agent_title(key))
            .child(div().text_color(colours::muted()).child(format!("{percent:.0}%")))
    }));
    [share, legend]
}

fn column(text: SharedString) -> Div {
    div().w(px(72.)).flex_none().flex().justify_end().child(text)
}

fn header() -> Div {
    div()
        .flex()
        .items_center()
        .px(px(8.))
        .pb(px(6.))
        .text_size(px(11.))
        .text_color(colours::faint())
        .child(div().flex_1().child("agent"))
        .children(COLUMNS.iter().map(|c| column((*c).into())))
}

fn agent_row(i: usize, key: &str, c: &Counts, most: f64) -> Stateful<Div> {
    let values = [
        data::duration(c.agent_seconds),
        data::count(c.agents),
        data::count(c.prompts),
        data::count(c.turns),
        data::duration(c.waited_seconds),
        data::tokens(c.tokens.out),
        data::tokens(c.tokens.sent()),
    ];
    let fraction = if most > 0. { data::ratio(c.agent_seconds, most) } else { 0. };
    div()
        .id(("usage-agent", i))
        .flex()
        .items_center()
        .px(px(8.))
        .py(px(8.))
        .rounded(px(4.))
        .hover(|s| s.bg(colours::hover()))
        .child(
            div()
                .flex_1()
                .min_w(px(140.))
                .pr(px(16.))
                .flex()
                .flex_col()
                .gap(px(5.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(swatch(agent_colour(key)))
                        .child(data::agent_title(key))
                        .child(ui::dim(format!("{} from cache", data::tokens(c.tokens.cache_read)))),
                )
                .child(bar(fraction, agent_colour(key), 4.)),
        )
        .children(values.into_iter().map(|v| column(v.into())))
}
