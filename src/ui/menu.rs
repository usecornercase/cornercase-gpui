use gpui::{
    App, Div, ElementId, FocusHandle, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent,
    ParentElement as _, Rgba, ScrollHandle, Stateful, StatefulInteractiveElement as _, Styled as _, Svg, Window,
    anchored, deferred, div, point, prelude::FluentBuilder as _, px,
};

use super::{Icon, glass, icon};
use crate::theme::{colours, radius};

pub(crate) const ABOVE: usize = 1;

pub(crate) struct Inks {
    pub value: Rgba,
    pub note: Rgba,
    pub mark: Rgba,
    pub alert: Rgba,
}

pub(crate) fn inks(lit: bool, dangerous: bool) -> Inks {
    if lit {
        let on = colours::on_brand();
        return Inks { value: on, note: glass::tint(on, 0.8), mark: on, alert: on };
    }
    let value = if dangerous { colours::failed() } else { colours::text() };
    Inks { value, note: colours::faint(), mark: colours::accent(), alert: colours::failed() }
}

pub(crate) fn step(highlight: Option<usize>, count: usize, down: bool) -> Option<usize> {
    let last = count.checked_sub(1)?;
    Some(match (highlight, down) {
        (None, true) => 0,
        (None, false) => last,
        (Some(at), true) => (at + 1).min(last),
        (Some(at), false) => at.saturating_sub(1),
    })
}

pub(crate) fn hovered(highlight: Option<usize>, index: usize, hovered: bool) -> Option<usize> {
    if hovered { Some(index) } else { highlight.filter(|at| *at != index) }
}

pub(crate) fn dropdown(id: impl Into<ElementId>, edge: Rgba) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(8.))
        .px(px(10.))
        .py(px(5.))
        .rounded(px(radius::control()))
        .text_size(px(12.))
        .bg(colours::hover())
        .border_1()
        .border_color(edge)
        .cursor_pointer()
        .hover(|s| s.bg(colours::selected()))
}

pub(crate) fn edge(open: bool, risky: bool) -> Rgba {
    match (open, risky) {
        (true, _) => colours::accent(),
        (false, true) => glass::tint(colours::failed(), 0.45),
        (false, false) => colours::border(),
    }
}

pub(crate) fn chevron() -> Svg {
    icon(Icon::ChevronDown, 12.).text_color(colours::muted())
}

pub(crate) fn backdrop(
    id: impl Into<ElementId>,
    window: &Window,
    close: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let size = window.viewport_size();
    let veil = div().id(id).w(size.width).h(size.height).occlude().on_any_mouse_down(close);
    deferred(anchored().position(point(px(0.), px(0.))).child(veil)).with_priority(1)
}

pub(crate) fn card(id: impl Into<ElementId>, focus: &FocusHandle, scroll: &ScrollHandle) -> Stateful<Div> {
    glass::card(radius::card())
        .id(id)
        .track_focus(focus)
        .track_scroll(scroll)
        .occlude()
        .overflow_y_scroll()
        .p(px(4.))
        .flex()
        .flex_col()
        .gap(px(1.))
        .text_size(px(12.5))
        .on_mouse_down(MouseButton::Left, |_: &MouseDownEvent, _, cx| cx.stop_propagation())
}

pub(crate) fn entry(id: impl Into<ElementId>, lit: bool, check: Option<Svg>) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_start()
        .gap(px(8.))
        .px(px(10.))
        .py(px(6.))
        .rounded(px(radius::control()))
        .cursor_pointer()
        .when(lit, |d| d.bg(glass::cobalt()))
        .child(div().w(px(13.)).pt(px(2.)).flex_none().children(check))
}

#[cfg(test)]
mod tests;
