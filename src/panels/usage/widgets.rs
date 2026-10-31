use gpui::{
    ClickEvent, Context, Div, FontWeight, Hsla, ParentElement as _, SharedString, StatefulInteractiveElement as _,
    Styled as _, div, px,
};

use super::UsagePanel;
use crate::theme::{colours, radius};
use crate::ui;

pub(super) fn card() -> Div {
    div().p(px(18.)).rounded(px(radius::CARD)).bg(colours::terminal()).border_1().border_color(colours::border())
}

pub(super) fn heading(text: impl Into<SharedString>) -> Div {
    div().text_size(px(11.)).text_color(colours::faint()).child(text.into().to_uppercase())
}

pub(super) fn error_line(text: impl Into<SharedString>) -> Div {
    div().text_size(px(12.)).text_color(colours::failed()).child(text.into())
}

pub(super) fn stale(error: Option<&String>) -> Option<Div> {
    error.map(|e| error_line(format!("could not refresh: {e}")))
}

pub(super) fn loading(text: &'static str) -> Div {
    card().child(ui::busy("usage-loading", text).text_size(px(13.)))
}

pub(super) fn failure(title: &'static str, error: &str, cx: &Context<UsagePanel>) -> Div {
    card()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(div().text_color(colours::failed()).font_weight(FontWeight::SEMIBOLD).child(title))
        .child(div().text_size(px(12.)).text_color(colours::muted()).child(error.to_string()))
        .child(
            div().child(
                ui::button("usage-retry", "Try again")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.refresh(true, cx))),
            ),
        )
}

pub(super) fn swatch(colour: impl Into<Hsla>) -> Div {
    div().size(px(8.)).rounded_full().bg(colour.into())
}

pub(super) fn plural(n: u64, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}
