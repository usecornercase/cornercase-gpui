use gpui::{
    ClickEvent, Context, Div, FontWeight, Hsla, ParentElement as _, SharedString, StatefulInteractiveElement as _,
    Styled as _, div, prelude::FluentBuilder as _, px, relative,
};

use super::UsagePanel;
use crate::theme::colours;
use crate::ui;

pub(super) fn card() -> Div {
    div().p(px(16.)).rounded(px(8.)).bg(colours::terminal()).border_1().border_color(colours::border())
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
    card().child(div().flex().items_center().gap(px(8.)).text_color(colours::muted()).child("◐").child(text))
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

pub(super) fn bar(fraction: f32, colour: impl Into<Hsla>, height: f32) -> Div {
    let fraction = fraction.clamp(0., 1.);
    div()
        .w_full()
        .h(px(height))
        .rounded(px(height / 2.))
        .bg(colours::border())
        .overflow_hidden()
        .when(fraction > 0., |d| {
            d.child(div().h_full().min_w(px(height)).w(relative(fraction)).rounded(px(height / 2.)).bg(colour.into()))
        })
}

pub(super) fn swatch(colour: impl Into<Hsla>) -> Div {
    div().size(px(8.)).rounded(px(4.)).bg(colour.into())
}

pub(super) fn plural(n: u64, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}
