//! Small building blocks every panel uses, so the window reads as one thing.

pub(crate) mod text_field;

use gpui::{
    AnyElement, Div, ElementId, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, Rgba,
    SharedString, Stateful, Styled as _, div, prelude::FluentBuilder as _, px,
};

pub(crate) use text_field::{TextField, TextFieldEvent};

use crate::theme::{colours, status_badge};

/// A bordered button. Add `.on_click(...)`.
pub(crate) fn button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    button_coloured(id, label, colours::muted())
}

/// The main action of a form or panel.
pub(crate) fn primary_button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    button_coloured(id, label, colours::accent()).border_color(colours::accent())
}

/// A button for something that cannot be undone.
pub(crate) fn danger_button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    button_coloured(id, label, colours::failed()).border_color(colours::failed())
}

pub(crate) fn button_coloured(id: impl Into<ElementId>, label: impl Into<SharedString>, colour: Rgba) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .px(px(8.))
        .py(px(3.))
        .rounded(px(5.))
        .text_size(px(12.))
        .text_color(colour)
        .border_1()
        .border_color(colours::border())
        .cursor_pointer()
        .whitespace_nowrap()
        .hover(|s| s.bg(colours::hover()))
        .child(label.into())
}

/// A borderless button with one glyph (✕, +, ⋯).
pub(crate) fn icon_button(id: impl Into<ElementId>, icon: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .px(px(5.))
        .rounded(px(4.))
        .text_color(colours::faint())
        .cursor_pointer()
        .hover(|s| s.bg(colours::selected()).text_color(colours::text()))
        .child(icon.into())
}

/// An agent state's icon in its colour, in a fixed-width slot.
pub(crate) fn badge(status: Option<&str>) -> Div {
    match status_badge(status) {
        Some((icon, colour)) => div().w(px(14.)).flex_none().text_color(colour).child(icon),
        None => div().w(px(14.)).flex_none(),
    }
}

/// A small upper-case heading over a group of rows.
pub(crate) fn section(title: impl Into<SharedString>) -> Div {
    let title: SharedString = title.into();
    div().pt(px(10.)).pb(px(4.)).text_size(px(11.)).text_color(colours::faint()).child(title.to_uppercase())
}

/// A hoverable, clickable row.
pub(crate) fn row(id: impl Into<ElementId>, selected: bool) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(8.))
        .px(px(8.))
        .py(px(4.))
        .rounded(px(5.))
        .cursor_pointer()
        .when(selected, |d| d.bg(colours::selected()))
        .hover(|s| s.bg(colours::hover()))
}

/// Grey text for empty lists, loading and errors.
pub(crate) fn note(text: impl Into<SharedString>) -> Div {
    div().p(px(12.)).text_color(colours::muted()).child(text.into())
}

/// Muted secondary text.
pub(crate) fn dim(text: impl Into<SharedString>) -> Div {
    div().text_size(px(11.)).text_color(colours::faint()).child(text.into())
}

/// A label over a control, for forms.
pub(crate) fn field(label: impl Into<SharedString>, control: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(div().text_size(px(11.)).text_color(colours::muted()).child(label.into()))
        .child(control)
}

/// The box a `TextField` sits in.
pub(crate) fn input_box(field: impl IntoElement, focused: bool) -> Div {
    div()
        .px(px(8.))
        .py(px(5.))
        .rounded(px(5.))
        .bg(colours::terminal())
        .border_1()
        .border_color(if focused { colours::accent() } else { colours::border() })
        .child(field)
}

/// A checkbox with its label. Add `.on_click(...)`.
pub(crate) fn checkbox(id: impl Into<ElementId>, checked: bool, label: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(8.))
        .cursor_pointer()
        .child(
            div()
                .size(px(14.))
                .flex_none()
                .rounded(px(3.))
                .border_1()
                .border_color(colours::accent())
                .when(checked, |d| d.bg(colours::accent())),
        )
        .child(label.into())
}

/// A row of tabs (segments); the selected one is highlighted. Each gets `.on_click` from the caller
/// through `make`.
pub(crate) fn segments(items: Vec<AnyElement>) -> Div {
    div().flex().gap(px(2.)).p(px(2.)).rounded(px(6.)).bg(colours::sidebar()).children(items)
}

pub(crate) fn segment(id: impl Into<ElementId>, label: impl Into<SharedString>, selected: bool) -> Stateful<Div> {
    div()
        .id(id)
        .px(px(10.))
        .py(px(3.))
        .rounded(px(5.))
        .text_size(px(12.))
        .cursor_pointer()
        .when(selected, |d| d.bg(colours::selected()).text_color(colours::text()))
        .when(!selected, |d| d.text_color(colours::muted()).hover(|s| s.bg(colours::hover())))
        .child(label.into())
}

/// Bold title text.
pub(crate) fn title(text: impl Into<SharedString>) -> Div {
    div().font_weight(FontWeight::SEMIBOLD).child(text.into())
}
