use std::cell::Cell;

use gpui::{
    AnyElement, AppContext as _, ClickEvent, Context, Entity, FocusHandle, Focusable as _, InteractiveElement as _,
    IntoElement, KeyDownEvent, MouseButton, ParentElement as _, StatefulInteractiveElement as _, Styled as _, Window,
    div, prelude::FluentBuilder as _, px, relative,
};
use serde_json::{Value, json};

use super::Browser;
use super::cdp::Event;
use crate::dialogs;
use crate::store::{Store, ToastKind};
use crate::theme::colours;
use crate::ui::{self, TextField, TextFieldEvent};

const LEAVE: &str = "Changes you made may not be saved.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Alert,
    Confirm,
    Prompt,
    BeforeUnload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Dialog {
    pub(crate) kind: Kind,
    pub(crate) message: String,
    pub(crate) origin: String,
    pub(crate) default: String,
    pub(crate) serial: u64,
}

pub(crate) fn dialog(params: &Value, serial: u64) -> Option<Dialog> {
    let kind = match params["type"].as_str()? {
        "alert" => Kind::Alert,
        "confirm" => Kind::Confirm,
        "prompt" => Kind::Prompt,
        "beforeunload" => Kind::BeforeUnload,
        _ => return None,
    };
    let text = |key: &str| params[key].as_str().unwrap_or_default().to_string();
    Some(Dialog {
        kind,
        message: text("message"),
        origin: origin(&text("url")),
        default: text("defaultPrompt"),
        serial,
    })
}

pub(crate) fn origin(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else { return String::new() };
    if !matches!(scheme, "http" | "https") {
        return String::new();
    }
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    host.rsplit_once('@').map_or(host, |(_, h)| h).to_string()
}

impl Dialog {
    pub(crate) fn title(&self) -> String {
        match (self.kind, self.origin.is_empty()) {
            (Kind::BeforeUnload, _) => "Leave this page?".to_string(),
            (_, true) => "This page says".to_string(),
            (_, false) => format!("{} says", self.origin),
        }
    }

    pub(crate) fn body(&self) -> &str {
        if self.kind == Kind::BeforeUnload { LEAVE } else { &self.message }
    }

    pub(crate) fn buttons(&self) -> (Option<&'static str>, &'static str) {
        match self.kind {
            Kind::Alert => (None, "OK"),
            Kind::Confirm | Kind::Prompt => (Some("Cancel"), "OK"),
            Kind::BeforeUnload => (Some("Stay"), "Leave"),
        }
    }
}

pub(crate) struct Card {
    field: Entity<TextField>,
    focus: FocusHandle,
    shown: Cell<u64>,
}

impl Card {
    fn new(window: &Window, cx: &mut Context<Browser>) -> Self {
        let field = cx.new(|cx| TextField::new("", cx));
        cx.subscribe_in(&field, window, |this, _, event, window, cx| match event {
            TextFieldEvent::Submit => this.answer(true, window, cx),
            TextFieldEvent::Cancel => this.answer(false, window, cx),
            TextFieldEvent::Changed => {}
        })
        .detach();
        Self { field, focus: cx.focus_handle(), shown: Cell::new(0) }
    }
}

impl Browser {
    pub(super) fn dialog_opening(&mut self, event: &Event) {
        let Some(target) = self.target_of(event) else { return };
        self.notices.serial += 1;
        if let Some(dialog) = dialog(&event.params, self.notices.serial) {
            self.notices.dialogs.insert(target, dialog);
        }
    }

    pub(super) fn dialog_closed(&mut self, event: &Event) {
        if let Some(target) = self.target_of(event) {
            self.notices.dialogs.remove(&target);
        }
    }

    fn answer(&mut self, accept: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(target) = self.tabs.active_target(self.workspace) else { return };
        let Some(dialog) = self.notices.dialogs.remove(&target) else { return };
        let Some(session) = self.tabs.get(&target).and_then(|t| t.session.clone()) else { return };
        let mut params = json!({ "accept": accept });
        if dialog.kind == Kind::Prompt {
            params["promptText"] = self.notices.card.get().map(|c| c.field.read(cx).text()).unwrap_or_default().into();
        }
        self.call((Some(&session), "Page.handleJavaScriptDialog", params), cx, |_, result, cx| {
            let Err(e) = result else { return };
            if !e.to_string().contains("No dialog is showing") {
                let text = format!("The page's dialog could not be answered: {e}");
                Store::get(cx).update(cx, |s, cx| s.toast(text, ToastKind::Error, cx));
            }
        });
        window.focus(&self.focus);
        cx.notify();
    }

    fn dialog_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let accept = match event.keystroke.key.as_str() {
            "enter" => true,
            "escape" => false,
            _ => return,
        };
        cx.stop_propagation();
        self.answer(accept, window, cx);
    }

    fn reveal(&self, card: &Card, dialog: &Dialog, window: &Window, cx: &mut Context<Self>) {
        if card.shown.replace(dialog.serial) == dialog.serial {
            return;
        }
        let prompt = dialog.kind == Kind::Prompt;
        if prompt {
            let default = dialog.default.clone();
            card.field.update(cx, |field, cx| field.set_text(default, cx));
        }
        let panel = self.panel_focus(cx);
        let engaged = panel.is_focused(window) || panel.contains_focused(window, cx);
        if window.focused(cx).is_some() && !engaged {
            return;
        }
        let handle = if prompt { card.field.read(cx).focus_handle(cx) } else { card.focus.clone() };
        cx.defer_in(window, move |_, window, _| window.focus(&handle));
    }

    pub(super) fn dialog_card(&self, dialog: &Dialog, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let card = self.notices.card.get_or_init(|| Card::new(window, cx));
        self.reveal(card, dialog, window, cx);
        let (field, focus) = (card.field.clone(), card.focus.clone());
        let body = div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(message(dialog.body()))
            .when(dialog.kind == Kind::Prompt, |d| d.child(ui::boxed(&field, window, cx)));
        let mouse_focus = focus.clone();
        let card = dialogs::frame(dialog.title(), body, dialog_buttons(dialog, cx))
            .w(px(400.))
            .max_w(relative(0.94))
            .track_focus(&focus)
            .on_key_down(cx.listener(Self::dialog_key))
            .on_mouse_down(MouseButton::Left, move |_, window, _| window.focus(&mouse_focus));
        div()
            .id("browser-dialog")
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .justify_center()
            .items_start()
            .pt(px(40.))
            .bg(colours::veil())
            .child(card)
            .into_any_element()
    }
}

fn message(text: &str) -> impl IntoElement + use<> {
    div()
        .id("browser-dialog-message")
        .max_h(px(220.))
        .overflow_y_scroll()
        .text_size(px(13.))
        .text_color(colours::soft())
        .child(text.to_string())
}

fn dialog_buttons(dialog: &Dialog, cx: &mut Context<Browser>) -> impl IntoElement + use<> {
    let (cancel, ok) = dialog.buttons();
    let ok = ui::primary_button("browser-dialog-ok", ok)
        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.answer(true, window, cx)));
    div()
        .flex()
        .gap(px(8.))
        .children(cancel.map(|label| {
            ui::button("browser-dialog-cancel", label)
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.answer(false, window, cx)))
        }))
        .child(ok)
}

pub(crate) fn dialog_mark() -> impl IntoElement {
    div().flex_none().size(px(7.)).rounded_full().bg(colours::waiting())
}

#[cfg(test)]
mod tests;
