use gpui::{Div, ElementId, FontWeight, IntoElement, SharedString, Stateful, div, prelude::*, px};

use crate::notify::tip;
use crate::theme::{colours, radius};
use crate::ui::{self, Icon, glass};

pub(super) const LEAD: f32 = 18.;
pub(super) const GAP: f32 = 12.;
pub(super) const ROW_X: f32 = 10.;
pub(super) const STATUS: f32 = 300.;
const ACTION_GAP: f32 = 4.;

pub(super) fn card() -> Div {
    div()
        .flex()
        .flex_col()
        .p(px(6.))
        .rounded(px(radius::CARD))
        .bg(colours::terminal())
        .border_1()
        .border_color(colours::border())
}

pub(super) fn header(glyph: Icon, title: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .h(px(40.))
        .px(px(ROW_X))
        .child(ui::icon(glyph, 15.).text_color(colours::accent()))
        .child(div().flex_none().text_size(px(13.5)).font_weight(FontWeight::SEMIBOLD).child(title.into()))
}

pub(super) fn rule() -> Div {
    div().h(px(1.)).mx(px(ROW_X)).mb(px(4.)).bg(colours::border())
}

pub(super) fn row(id: impl Into<ElementId>, hover: bool) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(GAP))
        .min_h(px(52.))
        .px(px(ROW_X))
        .py(px(8.))
        .rounded(px(radius::CONTROL))
        .when(hover, |d| d.hover(|s| s.bg(colours::hover())))
}

pub(super) fn lead(control: impl IntoElement) -> Div {
    div().w(px(LEAD)).flex_none().flex().justify_center().child(control)
}

pub(super) fn under_row(line: impl IntoElement) -> Div {
    div().pl(px(ROW_X + LEAD + GAP)).pr(px(ROW_X)).pb(px(8.)).child(line)
}

pub(super) fn radio(id: impl Into<ElementId>, selected: bool, hint: &'static str, live: bool) -> Stateful<Div> {
    let base = div()
        .id(id)
        .size(px(16.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .border_1()
        .tooltip(tip(hint));
    if selected {
        return base
            .bg(glass::cobalt())
            .border_color(colours::brand_light())
            .shadow(glass::glow(colours::brand(), 0.3, 6.))
            .child(div().size(px(6.)).rounded_full().bg(colours::on_brand()));
    }
    let base = base.border_color(colours::faint());
    if !live {
        return base.opacity(0.5);
    }
    base.cursor_pointer().hover(|s| s.border_color(colours::accent()).bg(glass::tint(colours::brand(), 0.18)))
}

pub(super) fn name_column(title: Div, detail: impl Into<SharedString>) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(3.))
        .child(title)
        .child(div().text_size(px(11.5)).text_color(colours::faint()).child(ui::one_line(detail)))
}

pub(super) fn title(name: impl Into<SharedString>) -> Div {
    div().flex().items_center().gap(px(8.)).child(
        div()
            .flex_none()
            .text_size(px(13.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(colours::text())
            .child(name.into()),
    )
}

pub(super) fn actions() -> Div {
    div().w(px(ui::ROW_ACTION * 2. + ACTION_GAP)).flex_none().flex().justify_end().gap(px(ACTION_GAP))
}

pub(super) fn slot() -> Div {
    div().size(px(ui::ROW_ACTION)).flex_none()
}

pub(super) fn note(text: impl Into<SharedString>) -> Div {
    div().text_size(px(12.)).text_color(colours::muted()).child(text.into())
}

pub(super) fn hint(text: impl Into<SharedString>) -> Div {
    div().text_size(px(11.5)).text_color(colours::faint()).child(text.into())
}

pub(super) fn form() -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .mx(px(ROW_X))
        .mt(px(4.))
        .mb(px(6.))
        .pt(px(12.))
        .border_t_1()
        .border_color(colours::border())
}

pub(super) fn form_line() -> Div {
    div().flex().flex_wrap().items_center().gap(px(10.))
}
