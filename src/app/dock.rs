use std::time::Duration;

use gpui::{
    AnyElement, AnyView, App, ClickEvent, Context, Div, Entity, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, StyleRefinement, Styled as _, div, px,
};

use crate::panels::PanelKind;
use crate::store::Store;
use crate::theme::{colours, radius};
use crate::ui::{self, motion};

use super::{DOCK_HEADER, GAP, Root, panel_title};

pub(super) mod key;

pub(super) fn over_the_terminal(dock: impl IntoElement, full: bool) -> AnyElement {
    if full { div().absolute().inset_0().child(dock).into_any_element() } else { dock.into_any_element() }
}

pub(super) fn dock_view(kind: PanelKind, view: AnyView, cx: &App) -> AnyView {
    if kind.drawn_cached(&view, cx) { view.cached(StyleRefinement::default().size_full()) } else { view }
}

impl Root {
    pub(crate) fn settle_dock(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(motion::ENTER + Duration::from_millis(20)).await;
            let _ = this.update(cx, Self::notify_dock);
        })
        .detach();
    }

    fn notify_dock(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.dock_view_id(cx) {
            App::notify(cx, id);
        }
    }
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
