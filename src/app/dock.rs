use gpui::{
    AnyElement, AnyView, ClickEvent, Div, Entity, IntoElement, ParentElement as _, StatefulInteractiveElement as _,
    Styled as _, div, px,
};

use crate::panels::PanelKind;
use crate::store::Store;
use crate::theme::{colours, radius};
use crate::ui;

use super::{DOCK_HEADER, GAP, panel_title};

pub(super) fn over_the_terminal(dock: impl IntoElement, full: bool) -> AnyElement {
    if full { div().absolute().inset_0().child(dock).into_any_element() } else { dock.into_any_element() }
}

pub(super) fn dock_card(kind: PanelKind, view: AnyView, store: &Entity<Store>, width: Option<f32>) -> Div {
    let card = div()
        .size_full()
        .flex()
        .flex_col()
        .rounded(px(radius::card()))
        .bg(colours::surface())
        .border_1()
        .border_color(colours::border())
        .child(dock_header(kind, store))
        .child(div().flex_1().overflow_hidden().child(view));
    let frame = width.map_or_else(|| div().size_full(), |width| div().w(px(width)).h_full().flex_none());
    frame.flex().pr(px(GAP)).child(card)
}

fn dock_header(kind: PanelKind, store: &Entity<Store>) -> Div {
    let store = store.clone();
    div()
        .h(px(DOCK_HEADER))
        .flex_none()
        .flex()
        .items_center()
        .justify_between()
        .pl(px(14.))
        .pr(px(10.))
        .border_b_1()
        .border_color(colours::border())
        .child(panel_title(kind))
        .child(ui::icon_button("dock-close", "✕").on_click(move |_: &ClickEvent, _, cx| {
            store.update(cx, Store::close_dock);
        }))
}
