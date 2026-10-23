use gpui::{Div, ElementId, FontWeight, IntoElement, SharedString, Stateful, div, prelude::*, px};

use crate::theme::colours;
use crate::ui;

pub(crate) fn nav() -> Div {
    div()
        .w(px(180.))
        .h_full()
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(1.))
        .p(px(10.))
        .bg(colours::sidebar())
        .border_r_1()
        .border_color(colours::border())
}

pub(crate) fn nav_item(id: impl Into<ElementId>, label: impl Into<SharedString>, selected: bool) -> Stateful<Div> {
    ui::row(id, selected)
        .py(px(5.))
        .text_color(if selected { colours::text() } else { colours::muted() })
        .child(label.into())
}

pub(crate) fn page(id: impl Into<ElementId>) -> Stateful<Div> {
    div().id(id).flex_1().h_full().overflow_y_scroll().px(px(22.)).pt(px(14.)).pb(px(28.))
}

pub(crate) fn heading(title: impl Into<SharedString>, about: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(3.))
        .pb(px(6.))
        .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).child(title.into()))
        .child(div().text_size(px(12.)).text_color(colours::muted()).child(about.into()))
}

pub(crate) fn card() -> Div {
    div()
        .flex()
        .flex_col()
        .px(px(14.))
        .rounded(px(8.))
        .bg(colours::terminal())
        .border_1()
        .border_color(colours::border())
}

pub(crate) fn row(first: bool, label: impl IntoElement, control: impl IntoElement) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(14.))
        .py(px(9.))
        .when(!first, |d| d.border_t_1().border_color(colours::border()))
        .child(div().flex_1().min_w(px(0.)).child(label))
        .child(div().flex_none().flex().items_center().gap(px(6.)).child(control))
}

pub(crate) fn label(text: impl Into<SharedString>, note: impl Into<SharedString>) -> Div {
    let note: SharedString = note.into();
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(div().child(text.into()))
        .when(!note.is_empty(), |d| d.child(div().text_size(px(11.5)).text_color(colours::faint()).child(note)))
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
        .w(px(30.))
        .h(px(17.))
        .flex_none()
        .flex()
        .items_center()
        .px(px(2.))
        .rounded(px(9.))
        .cursor_pointer()
        .bg(if on { colours::accent() } else { colours::selected() })
        .when(on, Styled::justify_end)
        .child(div().size(px(13.)).rounded(px(7.)).bg(if on { colours::terminal() } else { colours::muted() }))
}

pub(crate) fn error(text: impl Into<SharedString>) -> Div {
    div().pb(px(8.)).text_size(px(12.)).text_color(colours::failed()).child(text.into())
}

pub(crate) fn confirm(question: impl Into<SharedString>, cancel: Stateful<Div>, go: Stateful<Div>) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .mb(px(9.))
        .px(px(10.))
        .py(px(7.))
        .rounded(px(6.))
        .bg(colours::panel())
        .border_1()
        .border_color(colours::failed())
        .child(div().flex_1().text_size(px(12.)).child(question.into()))
        .child(cancel)
        .child(go)
}

pub(crate) fn missing(text: impl Into<SharedString>, issue: u32) -> Div {
    div()
        .text_size(px(12.))
        .text_color(colours::muted())
        .child(format!("{} (needs usecornercase/cornercase-terminal#{issue})", text.into()))
}

#[cfg(test)]
mod tests;
