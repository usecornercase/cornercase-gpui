use gpui::{Div, ElementId, FontWeight, IntoElement, Rgba, ScrollHandle, SharedString, Stateful, div, prelude::*, px};

use crate::theme::{colours, radius};
use crate::ui::{self, Icon, glass};

pub(crate) const SLOT: f32 = ui::ROW_ACTION;
const LABEL: f32 = 230.;
const CONTENT: f32 = 800.;

pub(crate) fn nav(width: f32) -> Div {
    div()
        .w(px(width))
        .h_full()
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(2.))
        .p(px(12.))
        .rounded_bl(px(radius::SHEET - 1.))
        .bg(colours::sidebar())
        .border_r_1()
        .border_color(colours::border())
}

pub(crate) fn nav_item(
    id: impl Into<ElementId>,
    glyph: Icon,
    label: impl Into<SharedString>,
    selected: bool,
) -> Stateful<Div> {
    let (ink, tint) = if selected { (colours::text(), colours::accent()) } else { (colours::soft(), colours::muted()) };
    ui::row(id, selected)
        .gap(px(10.))
        .py(px(6.))
        .text_color(ink)
        .child(ui::icon(glyph, 15.).text_color(tint))
        .child(div().flex_1().min_w_0().child(label.into()))
}

pub(crate) fn nav_count(count: usize) -> Div {
    div().flex_none().text_size(px(11.)).text_color(colours::muted()).child(count.to_string())
}

pub(crate) fn page(id: impl Into<ElementId>, scroll: &ScrollHandle) -> Stateful<Div> {
    div().id(id).flex_1().min_w_0().h_full().overflow_y_scroll().track_scroll(scroll)
}

pub(crate) fn column() -> Div {
    div().w_full().max_w(px(CONTENT)).mx_auto().px(px(28.)).pt(px(22.)).pb(px(40.)).flex().flex_col().gap(px(20.))
}

pub(crate) fn heading(glyph: Icon, title: impl Into<SharedString>, about: impl Into<SharedString>) -> Div {
    let about: SharedString = about.into();
    div()
        .flex()
        .flex_col()
        .gap(px(5.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(ui::icon(glyph, 18.).text_color(colours::accent()))
                .child(div().text_size(px(17.)).font_weight(FontWeight::SEMIBOLD).child(title.into())),
        )
        .when(!about.is_empty(), |d| d.child(div().text_size(px(12.5)).text_color(colours::muted()).child(about)))
}

pub(crate) fn caption(title: impl Into<SharedString>) -> Div {
    let title: SharedString = title.into();
    div()
        .pb(px(7.))
        .pl(px(2.))
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(colours::muted())
        .child(title.to_uppercase())
}

pub(crate) fn group(title: impl Into<SharedString>, body: impl IntoElement) -> Div {
    let title: SharedString = title.into();
    div().flex().flex_col().when(!title.is_empty(), |d| d.child(caption(title))).child(body)
}

pub(crate) fn card() -> Div {
    div()
        .flex()
        .flex_col()
        .px(px(16.))
        .rounded(px(radius::CARD))
        .bg(colours::terminal())
        .border_1()
        .border_color(colours::border())
}

pub(crate) fn row(first: bool, label: impl IntoElement, control: impl IntoElement, slot: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap_x(px(14.))
        .gap_y(px(8.))
        .min_h(px(54.))
        .py(px(10.))
        .when(!first, |d| d.border_t_1().border_color(colours::border()))
        .child(div().flex_grow().flex_basis(px(LABEL)).min_w_0().child(label))
        .child(div().flex_none().flex().items_center().gap(px(8.)).child(control))
        .child(div().w(px(SLOT)).flex_none().flex().justify_center().child(slot))
}

pub(crate) fn label(text: impl Into<SharedString>, note: impl Into<SharedString>) -> Div {
    labelled(div().child(text.into()), note)
}

pub(crate) fn labelled(title: impl IntoElement, note: impl Into<SharedString>) -> Div {
    let note: SharedString = note.into();
    div()
        .flex()
        .flex_col()
        .gap(px(3.))
        .child(title)
        .when(!note.is_empty(), |d| d.child(div().text_size(px(11.5)).text_color(colours::faint()).child(note)))
}

pub(crate) fn titled(text: impl Into<SharedString>, extra: impl IntoIterator<Item = Div>) -> Div {
    div().flex().flex_wrap().items_center().gap(px(8.)).child(text.into()).children(extra)
}

pub(crate) fn status(text: impl Into<SharedString>, colour: Option<Rgba>, ink: Rgba) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .text_size(px(11.5))
        .text_color(ink)
        .child(ui::dot(colour))
        .child(text.into())
}

pub(crate) fn under(line: impl IntoElement) -> Div {
    div().pb(px(10.)).child(line)
}

pub(crate) fn hint(text: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .items_start()
        .gap(px(6.))
        .text_size(px(11.5))
        .text_color(colours::muted())
        .child(div().pt(px(2.)).child(ui::icon(Icon::Help, 12.).text_color(colours::faint())))
        .child(div().flex_1().min_w_0().child(text.into()))
}

pub(crate) fn capitalised(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

pub(crate) fn switch(id: impl Into<ElementId>, on: bool) -> Stateful<Div> {
    div()
        .id(id)
        .w(px(34.))
        .h(px(20.))
        .flex_none()
        .flex()
        .items_center()
        .px(px(2.))
        .rounded_full()
        .cursor_pointer()
        .when(on, |d| d.bg(glass::cobalt()).shadow(glass::glow(colours::brand(), 0.3, 8.)).justify_end())
        .when(!on, |d| d.bg(colours::selected()))
        .child(div().size(px(16.)).rounded_full().bg(if on { colours::on_brand() } else { colours::soft() }))
}

pub(crate) fn dropdown(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    risky: bool,
    open: bool,
) -> Stateful<Div> {
    let ink = if risky { colours::failed() } else { colours::text() };
    let edge = match (open, risky) {
        (true, _) => colours::accent(),
        (false, true) => glass::tint(colours::failed(), 0.45),
        (false, false) => colours::border(),
    };
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(8.))
        .min_w(px(120.))
        .max_w(px(260.))
        .px(px(10.))
        .py(px(5.))
        .rounded(px(radius::CONTROL))
        .text_size(px(12.))
        .text_color(ink)
        .bg(colours::hover())
        .border_1()
        .border_color(edge)
        .cursor_pointer()
        .hover(|s| s.bg(colours::selected()))
        .children(risky.then(|| ui::icon(Icon::Alert, 12.).text_color(colours::failed())))
        .child(div().flex_1().min_w_0().flex().child(ui::one_line(label)))
        .child(ui::icon(Icon::ChevronDown, 12.).text_color(colours::muted()))
}

#[cfg(test)]
mod tests;
