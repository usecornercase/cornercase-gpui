use std::cell::Cell;
use std::rc::Rc;

use gpui::{App, Bounds, ClipboardItem, Context, Focusable as _, Pixels, Window};
use serde_json::{Value, json};

use super::Browser;
use super::agent::{Agents, Menu};
use super::cdp::Event;
use super::picker::Pick;
use super::pointer::Pointer;
use super::select::Choice;
use super::typing::Typing;
use super::viewport::Rect;
use crate::escape::{self, Top};
use crate::keymap::Command;

#[derive(Default)]
pub(crate) struct Input {
    pub(crate) pointer: Pointer,
    pub(crate) typing: Typing,
    pub(crate) choice: Option<Choice>,
    pub(crate) bounds: Rc<Cell<Bounds<Pixels>>>,
    pub(crate) page_waits: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum KeysGo {
    Stay,
    Page,
    Address,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeysAt {
    Viewport,
    Address,
    Elsewhere,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Waiting {
    No,
    Same,
    Edited,
}

pub(crate) fn keys_go(at: KeysAt, page_shown: bool, waiting: Waiting) -> (KeysGo, bool) {
    match (at, waiting) {
        (KeysAt::Viewport, _) if !page_shown => (KeysGo::Address, false),
        (KeysAt::Address, Waiting::Same) if page_shown => (KeysGo::Page, false),
        (KeysAt::Address, Waiting::Same) => (KeysGo::Stay, true),
        _ => (KeysGo::Stay, false),
    }
}

pub(crate) fn agents_menu_on_top(agents: &Agents, many: bool) -> bool {
    let card = matches!(agents.pick, Some(Pick::Picked(_))) && many;
    agents.menu.is_some_and(|menu| menu != Menu::Chooser || card)
}

pub(crate) fn close_menu_on_top(agents: &mut Agents, devices: &mut bool, many: bool) {
    let on_top = agents_menu_on_top(agents, many);
    agents.menu = None;
    if !on_top {
        *devices = false;
    }
}

pub(crate) fn escape_tops(choice: bool, agents: &Agents, devices: bool, many: bool) -> Vec<Top> {
    let menu = agents_menu_on_top(agents, many) || devices;
    escape::tops_of(&[(Top::Choice, choice), (Top::Menu, menu), (Top::Pick, agents.pick.is_some())])
}

pub(crate) fn selection_script(cut: bool) -> Value {
    let helper = if cut { "cut" } else { "selected" };
    let expression =
        format!("window.__cornercase && window.__cornercase.{helper} ? window.__cornercase.{helper}() : ''");
    json!({ "expression": expression, "returnByValue": true })
}

pub(crate) fn evaluated_text(reply: &Value) -> Option<String> {
    reply["result"]["value"].as_str().filter(|text| !text.is_empty()).map(str::to_string)
}

impl Browser {
    pub(super) fn input_message(&mut self, message: &Value, event: &Event, cx: &mut Context<Self>) {
        match message["kind"].as_str() {
            Some("caret") => self.input.typing.caret = Rect::read(message),
            Some("select") => self.open_choice(message, event, cx),
            _ => {}
        }
    }

    pub(crate) fn page_focused(&self, window: &Window) -> bool {
        self.shown && self.focus.is_focused(window)
    }

    pub(crate) fn has_focus(&self, window: &Window, cx: &App) -> bool {
        self.page_focused(window) || (self.shown && self.address.read(cx).focus_handle(cx).is_focused(window))
    }

    pub(super) fn keep_the_keys(&mut self, window: &Window, cx: &mut Context<Self>) {
        let address = self.address.read(cx).focus_handle(cx);
        let at = if self.focus.is_focused(window) {
            KeysAt::Viewport
        } else if address.is_focused(window) {
            KeysAt::Address
        } else {
            KeysAt::Elsewhere
        };
        let typed = self.address.read(cx).text();
        let waiting = match &self.input.page_waits {
            None => Waiting::No,
            Some(text) if *text == typed => Waiting::Same,
            Some(_) => Waiting::Edited,
        };
        let (go, waits) = keys_go(at, self.page_shown(), waiting);
        if !waits {
            self.input.page_waits = None;
        }
        match go {
            KeysGo::Stay => {}
            KeysGo::Page => cx.defer_in(window, |this, window, _| window.focus(&this.focus)),
            KeysGo::Address => cx.defer_in(window, move |_, window, _| window.focus(&address)),
        }
    }

    pub(crate) fn escape(&mut self) {
        self.press_escape();
    }

    pub(crate) fn tops(&self, cx: &App) -> Vec<Top> {
        let many = self.many_agents(cx);
        escape_tops(self.input.choice.is_some(), &self.agents, self.devices_open, many)
    }

    pub(crate) fn close_top(&mut self, top: Top, cx: &mut Context<Self>) {
        match top {
            Top::Choice => self.close_choice(cx),
            Top::Menu => {
                let many = self.many_agents(cx);
                close_menu_on_top(&mut self.agents, &mut self.devices_open, many);
                cx.notify();
            }
            Top::Pick => {
                self.cancel_pick(cx);
            }
            Top::Find | Top::Selection => {}
        }
    }

    pub(crate) fn command(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let page = self.page_focused(window);
        match command {
            Command::Back => self.back(cx),
            Command::Forward => self.forward(cx),
            Command::Reload => self.reload(cx),
            Command::FocusAddress => self.focus_address(window, cx),
            Command::Copy if page => self.copy_selection(false, cx),
            Command::Cut if page => self.copy_selection(true, cx),
            Command::Paste if page => self.paste(cx),
            _ => return false,
        }
        true
    }

    fn copy_selection(&mut self, cut: bool, cx: &mut Context<Self>) {
        let Some(session) = self.active_session() else { return };
        self.call((Some(&session), "Runtime.evaluate", selection_script(cut)), cx, |_, reply, cx| {
            if let Some(text) = reply.ok().as_ref().and_then(evaluated_text) {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
            }
        });
    }

    fn paste(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.tell_page(("Input.insertText", json!({ "text": text })));
        }
    }
}

#[cfg(test)]
mod tests;
