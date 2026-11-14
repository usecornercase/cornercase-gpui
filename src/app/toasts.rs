use gpui::{
    AnyElement, ClickEvent, Context, Div, Entity, InteractiveElement as _, IntoElement, ParentElement as _, Rgba,
    SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

use super::frame::Frame;
use super::{GAP, Root};
use crate::notify;
use crate::notify::update::Updating;
use crate::self_update;
use crate::store::{Store, Toast, ToastKind};
use crate::theme::{colours, radius};
use crate::ui;
use crate::ui::Fit;
use crate::ui::motion::{self, Way};

const TOAST_SLIDE: f32 = 28.;
const WIDTH: f32 = 348.;
const RIGHT: f32 = 16.;
const BOTTOM: f32 = 40.;

impl Root {
    pub(super) fn render_toasts(&self, window_frame: Frame, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let toasts: Vec<_> = self.store.read(cx).toasts.iter().filter(|t| !t.expired()).cloned().collect();
        let ids: Vec<u64> = toasts.iter().map(|t| t.id).collect();
        let store = self.store.clone();
        let tell = store.clone();
        let stack = toasts
            .into_iter()
            .fold(ui::stack(GAP), |stack, toast| stack.item(card(&store, toast)))
            .more(waiting)
            .on_drawn(move |drawn, cx| {
                if !ids.is_empty() {
                    tell.update(cx, |s, cx| s.toasts_drawn(&ids[drawn], cx));
                }
            });
        div()
            .absolute()
            .top(px(window_frame.title_bottom() + GAP))
            .bottom(px(BOTTOM))
            .right(px(RIGHT))
            .w(px(WIDTH))
            .child(stack)
    }

    pub(super) fn open_update(&self, updating: Updating, window: &mut Window, cx: &mut Context<Self>) {
        match updating {
            Updating::Cornercase => notify::update::show_update(self.notifier.clone(), window, cx),
            Updating::Window => self_update::show_dialog(window, cx),
        }
    }
}

fn toast_colour(kind: ToastKind) -> Rgba {
    match kind {
        ToastKind::Info => colours::accent(),
        ToastKind::Error => colours::failed(),
        ToastKind::NeedsYou => colours::waiting(),
        ToastKind::Done => colours::done(),
    }
}

fn card(store: &Entity<Store>, toast: Toast) -> impl Fn(Fit) -> AnyElement + 'static {
    let store = store.clone();
    move |fit| {
        let view = toast_view(&store, toast.clone(), fit.most);
        if fit.measuring {
            view.into_any_element()
        } else {
            motion::slide(view, ("toast-in", toast.id), Way::In, (TOAST_SLIDE, 0.)).into_any_element()
        }
    }
}

fn waiting(n: usize) -> AnyElement {
    div()
        .flex()
        .justify_end()
        .child(
            ui::glass::card(radius::chip())
                .occlude()
                .px(px(8.))
                .py(px(2.))
                .text_size(px(11.))
                .text_color(colours::muted())
                .child(format!("+{n} more")),
        )
        .into_any_element()
}

fn toast_view(store: &Entity<Store>, toast: Toast, most: f32) -> Stateful<Div> {
    let id = toast.id;
    let (go, dismiss) = (store.clone(), store.clone());
    let colour = toast_colour(toast.kind);
    ui::glass::card(radius::card())
        .id(SharedString::from(format!("toast-{id}")))
        .w(px(348.))
        .flex()
        .items_center()
        .gap(px(10.))
        .px(px(14.))
        .py(px(10.))
        .occlude()
        .child(
            div().w(px(3.)).h(px(18.)).rounded_full().bg(colour).shadow(ui::glass::glow(colour, 0.6, 8.)).flex_none(),
        )
        .child(
            div()
                .id(("toast-text", id))
                .flex_1()
                .min_w_0()
                .max_h(px(most))
                .overflow_y_scroll()
                .text_size(px(12.))
                .child(toast.text),
        )
        .when(toast.count > 1, |d| d.child(toast_count(toast.count)))
        .when_some(toast.action, move |d, (label, action)| {
            d.child(ui::primary_button(SharedString::from(format!("toast-go-{id}")), label).on_click(
                move |_: &ClickEvent, _, cx| {
                    go.update(cx, |s, cx| {
                        s.go(&action, cx);
                        s.dismiss_toast(id, cx);
                    });
                },
            ))
        })
        .child(
            ui::icon_button(SharedString::from(format!("toast-x-{id}")), "✕")
                .on_click(move |_: &ClickEvent, _, cx| dismiss.update(cx, |s, cx| s.dismiss_toast(id, cx))),
        )
}

fn toast_count(count: u32) -> impl IntoElement {
    div()
        .flex_none()
        .px(px(6.))
        .rounded(px(radius::chip()))
        .bg(colours::selected())
        .text_size(px(11.))
        .text_color(colours::muted())
        .child(format!("×{count}"))
}

#[cfg(test)]
mod tests;
