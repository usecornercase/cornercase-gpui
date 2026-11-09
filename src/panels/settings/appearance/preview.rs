use gpui::{Div, ParentElement as _, Rgba, Styled as _, div, px};

use crate::theme::colour::{from_bytes, opaque, over};
use crate::theme::{FONT_FAMILY, Theme};

pub(super) const WIDTH: f32 = 156.;
pub(super) const HEIGHT: f32 = 96.;

fn bar(width: f32, colour: Rgba) -> Div {
    div().w(px(width)).h(px(3.)).rounded_full().bg(colour)
}

fn sidebar(theme: &Theme, ground: Rgba) -> Div {
    let card = over(Rgba { a: theme.opacity.cards.max(0.4), ..theme.chrome.surface }, ground);
    let rows = [theme.chrome.working, theme.chrome.waiting, theme.chrome.done].map(|status| {
        div()
            .flex()
            .items_center()
            .gap(px(3.))
            .child(div().size(px(4.)).rounded_full().bg(status))
            .child(bar(16., over(theme.chrome.muted, card)))
    });
    div().w(px(38.)).h_full().p(px(5.)).flex().flex_col().gap(px(5.)).rounded(px(5.)).bg(card).children(rows)
}

fn run(text: &'static str, colour: [u8; 3]) -> Div {
    div().text_color(from_bytes(colour)).child(text)
}

fn pane(theme: &Theme) -> Div {
    let terminal = &theme.terminal;
    let prompt = div()
        .flex()
        .gap(px(4.))
        .child(run("~", terminal.ansi[4]))
        .child(run("$", terminal.ansi[2]))
        .child(run("ls", terminal.foreground));
    let found = div().flex().gap(px(4.)).child(run("src", terminal.ansi[4])).child(run("error", terminal.ansi[1]));
    let warn = div().flex().gap(px(4.)).child(run("warn", terminal.ansi[3])).child(run("done", terminal.ansi[2]));
    div()
        .flex_1()
        .h_full()
        .p(px(6.))
        .flex()
        .flex_col()
        .gap(px(2.))
        .rounded(px(5.))
        .bg(from_bytes(terminal.background))
        .font_family(FONT_FAMILY)
        .text_size(px(8.))
        .child(prompt)
        .child(found)
        .child(warn)
        .child(
            div().mt(px(4.)).w(px(24.)).h(px(8.)).rounded_full().bg(theme.chrome.brand).child(div().w_full().h_full()),
        )
}

pub(super) fn preview(theme: &Theme) -> Div {
    let ground = opaque(theme.chrome.glass);
    div()
        .w(px(WIDTH))
        .h(px(HEIGHT))
        .p(px(5.))
        .flex()
        .gap(px(4.))
        .rounded(px(8.))
        .bg(ground)
        .border_1()
        .border_color(theme.chrome.edge)
        .child(sidebar(theme, ground))
        .child(pane(theme))
}
