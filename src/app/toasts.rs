use gpui::{
    ClickEvent, Context, Div, Entity, InteractiveElement as _, IntoElement, ParentElement as _, Rgba, SharedString,
    Stateful, StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _, px,
};

use super::Root;
use crate::store::{Store, Toast, ToastGo, ToastKind};
use crate::theme::{colours, radius};
use crate::ui;
use crate::ui::motion::{self, Way};

const TOAST_SLIDE: f32 = 28.;

impl Root {
    pub(super) fn render_toasts(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let toasts: Vec<_> = self.store.read(cx).toasts.iter().filter(|t| !t.expired()).cloned().collect();
        let store = self.store.clone();
        div().absolute().bottom(px(40.)).right(px(16.)).flex().flex_col().gap(px(8.)).children(toasts.into_iter().map(
            move |t| {
                let id = t.id;
                motion::slide(toast_view(&store, t), ("toast-in", id), Way::In, (TOAST_SLIDE, 0.))
            },
        ))
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

fn toast_view(store: &Entity<Store>, toast: Toast) -> Stateful<Div> {
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
        .child(div().flex_1().min_w_0().text_size(px(12.)).child(toast.text))
        .when(toast.count > 1, |d| d.child(toast_count(toast.count)))
        .when_some(toast.action, move |d, (label, action)| {
            d.child(ui::primary_button(SharedString::from(format!("toast-go-{id}")), label).on_click(
                move |_: &ClickEvent, _, cx| {
                    go.update(cx, |s, cx| {
                        match &action {
                            ToastGo::Run(args) => s.act(args.clone()),
                            ToastGo::Machine(there) => s.go_there(there, cx),
                        }
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
