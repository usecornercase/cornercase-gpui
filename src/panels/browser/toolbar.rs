use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, App, ClickEvent, Context, Div, ElementId, Entity, Focusable as _,
    InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent, MouseUpEvent, ParentElement as _, Stateful,
    StatefulInteractiveElement as _, Styled as _, Window, div, ease_in_out, prelude::FluentBuilder as _, px, relative,
};
use serde_json::json;

use super::{Browser, Status, shown_address};
use crate::notify::tip;
use crate::theme::colours;
use crate::ui::text_field::{End, SelectAll, TextField, TextFieldEvent};
use crate::ui::{self, Icon, glass};

pub(super) const BUTTON: f32 = 28.;
const TOOLBAR: f32 = 44.;
const INLINE: f32 = 22.;
const SWEEP: Duration = Duration::from_millis(1100);
const SWEEP_SHARE: f32 = 0.3;

type Act = fn(&mut Browser, &mut Context<Browser>);

struct Nav {
    id: &'static str,
    glyph: Icon,
    label: &'static str,
    act: Act,
}

const BACK: Nav = Nav { id: "browser-back", glyph: Icon::ArrowLeft, label: "Back", act: Browser::back };
const FORWARD: Nav = Nav { id: "browser-forward", glyph: Icon::ArrowRight, label: "Forward", act: Browser::forward };
const RELOAD: Nav = Nav { id: "browser-reload", glyph: Icon::Reload, label: "Reload", act: Browser::reload };
const STOP: Nav = Nav { id: "browser-stop", glyph: Icon::Close, label: "Stop loading", act: Browser::stop };

pub(crate) fn outside_url(url: &str) -> Option<&str> {
    let url = url.trim();
    let scheme = url.split_once("://").map(|(scheme, _)| scheme)?;
    matches!(scheme, "http" | "https" | "file").then_some(url)
}

impl Browser {
    pub(super) fn toolbar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let ready = matches!(self.status, Status::Ready(_));
        div()
            .relative()
            .h(px(TOOLBAR))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(6.))
            .px(px(8.))
            .border_b_1()
            .border_color(colours::border())
            .child(self.navigation(cx))
            .child(self.address_box(window, cx))
            .child(self.tools(cx))
            .child(new_tab_button(ready, cx))
            .when(self.loading(), |d| d.child(loading_bar()))
    }

    fn navigation(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let tab = self.tabs.active(self.workspace);
        let (back, forward) = tab.map_or((false, false), |t| (t.back.is_some(), t.forward.is_some()));
        let reload = if self.loading() { &STOP } else { &RELOAD };
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(2.))
            .child(nav_button(&BACK, back, cx))
            .child(nav_button(&FORWARD, forward, cx))
            .child(nav_button(reload, tab.is_some(), cx))
    }

    fn tools(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(2.))
            .child(self.device_slot(cx))
            .child(self.pick_button(cx))
            .child(self.shots_button(cx))
            .child(self.agents_button(cx))
            .child(self.console_button(cx))
    }

    fn address_box(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let focused = self.address.read(cx).focus_handle(cx).is_focused(window);
        let field = div()
            .flex_1()
            .min_w_0()
            .capture_any_mouse_down(cx.listener(|this, _: &MouseDownEvent, window, cx| {
                this.address_pressed = !this.address.read(cx).focus_handle(cx).is_focused(window);
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, window, cx| {
                    if std::mem::take(&mut this.address_pressed) {
                        this.select_address(window, cx);
                    }
                }),
            )
            .child(self.address.clone());
        let open = self.active_url().as_deref().and_then(outside_url).map(|url| open_outside(url.to_string()));
        let inner = div().h(px(INLINE)).flex().items_center().gap(px(4.)).child(field).children(open);
        div().flex_1().min_w_0().child(ui::input_box(inner, focused).py(px(4.)).pr(px(4.)))
    }

    pub(super) fn loading(&self) -> bool {
        self.tabs.active(self.workspace).is_some_and(|t| t.loading)
    }

    pub(super) fn check_loading(&mut self, session: &str, cx: &mut Context<Self>) {
        let ask = json!({ "expression": "document.readyState", "returnByValue": true });
        let asked = session.to_string();
        self.call((Some(session), "Runtime.evaluate", ask), cx, move |this, result, cx| {
            if !result.is_ok_and(|r| r["result"]["value"] == "complete") {
                return;
            }
            if let Some(tab) =
                this.tabs.by_session(&asked).map(|t| t.target.clone()).and_then(|t| this.tabs.get_mut(&t))
            {
                tab.loading = false;
            }
            this.refresh_title(Some(&asked), cx);
            cx.notify();
        });
    }

    fn stop(&mut self, _cx: &mut Context<Self>) {
        if let Some(session) = self.active_session() {
            self.send(Some(&session), "Page.stopLoading", json!({}));
        }
    }

    fn new_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_tab("about:blank", self.workspace, cx);
        self.address_url = "about:blank".to_string();
        self.address.update(cx, TextField::clear);
        window.focus(&self.address.read(cx).focus_handle(cx));
        cx.notify();
    }

    fn address_event(
        &mut self,
        field: &Entity<TextField>,
        event: &TextFieldEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            TextFieldEvent::Submit => {
                let text = field.read(cx).text();
                self.go_to(&text, cx);
                window.focus(&self.focus);
            }
            TextFieldEvent::Cancel => {
                let url = self.active_url().unwrap_or_default();
                field.update(cx, |field, cx| field.set_text(shown_address(&url), cx));
                self.select_address(window, cx);
            }
            TextFieldEvent::Changed => {}
        }
    }

    pub(super) fn watch_address(address: &Entity<TextField>, window: &mut Window, cx: &mut Context<Self>) {
        cx.subscribe_in(address, window, Self::address_event).detach();
        let focus = address.read(cx).focus_handle(cx);
        cx.on_focus(&focus, window, |this, window, cx| this.select_address(window, cx)).detach();
        cx.on_blur(&focus, window, |this, window, cx| {
            this.address.read(cx).focus_handle(cx).dispatch_action(&End, window, cx);
        })
        .detach();
    }

    fn select_address(&self, window: &mut Window, cx: &mut App) {
        self.address.read(cx).focus_handle(cx).dispatch_action(&SelectAll, window, cx);
    }
}

fn nav_button(nav: &Nav, enabled: bool, cx: &mut Context<Browser>) -> Stateful<Div> {
    if !enabled {
        return still(nav.id, nav.glyph);
    }
    let act = nav.act;
    ui::tool_button(nav.id, nav.glyph, BUTTON, false)
        .tooltip(tip(nav.label))
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| act(this, cx)))
}

fn new_tab_button(ready: bool, cx: &mut Context<Browser>) -> Stateful<Div> {
    if !ready {
        return still("browser-new-tab", Icon::Plus);
    }
    ui::tool_button("browser-new-tab", Icon::Plus, BUTTON, false)
        .tooltip(tip("New tab"))
        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.new_tab(window, cx)))
}

fn still(id: impl Into<ElementId>, glyph: Icon) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .size(px(BUTTON))
        .child(ui::icon(glyph, (BUTTON * 0.5).round()).text_color(colours::faint()).opacity(0.45))
}

fn open_outside(url: String) -> Stateful<Div> {
    ui::tool_button("browser-open-outside", Icon::External, INLINE, false)
        .tooltip(tip("Open in the system browser"))
        .on_click(move |_: &ClickEvent, _, cx| cx.open_url(&url))
}

fn loading_bar() -> impl IntoElement {
    let sweep = div()
        .absolute()
        .top_0()
        .h_full()
        .w(relative(SWEEP_SHARE))
        .rounded_full()
        .bg(colours::brand_light())
        .shadow(glass::glow(colours::brand(), 0.7, 6.));
    let moving =
        sweep.with_animation("browser-loading", Animation::new(SWEEP).repeat().with_easing(ease_in_out), |sweep, t| {
            sweep.left(relative(t * (1. + SWEEP_SHARE) - SWEEP_SHARE))
        });
    div().absolute().left_0().right_0().bottom(px(-1.)).h(px(2.)).overflow_hidden().child(moving)
}

#[cfg(test)]
mod tests;
