use std::cell::RefCell;

use gpui::{
    ClickEvent, Div, FocusHandle, InteractiveElement as _, KeyDownEvent, Modifiers, MouseButton, SharedString,
    Stateful, StatefulInteractiveElement as _, Styled as _,
};

use super::glass;
use crate::theme::colours;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Key {
    Press,
    Next,
    Previous,
    Other,
}

pub(crate) fn key_of(key: &str, modifiers: Modifiers) -> Key {
    let shift_alone = modifiers == Modifiers { shift: true, ..Modifiers::default() };
    match key {
        "enter" | "space" if !modifiers.modified() => Key::Press,
        "tab" if !modifiers.modified() => Key::Next,
        "tab" if shift_alone => Key::Previous,
        _ => Key::Other,
    }
}

#[derive(Debug, Default)]
pub(crate) struct Arm {
    name: Option<SharedString>,
    live: bool,
    before: bool,
}

impl Arm {
    pub(crate) fn keystroke(&mut self) {
        self.before = self.live;
        self.live = false;
    }

    pub(crate) fn down(&mut self, name: &SharedString, held: bool) {
        if !held {
            self.name = Some(name.clone());
            self.live = true;
        } else if self.name.as_ref() == Some(name) {
            self.live = self.before;
        }
    }

    pub(crate) fn up(&mut self, name: &str) -> bool {
        let pressed = self.live && self.name.as_ref().is_some_and(|armed| armed.as_ref() == name);
        *self = Self::default();
        pressed
    }
}

thread_local! {
    static ARM: RefCell<Arm> = RefCell::new(Arm::default());
}

pub(crate) fn keystroke() {
    ARM.with_borrow_mut(Arm::keystroke);
}

pub(crate) fn pressed(event: &ClickEvent, name: &str) -> bool {
    !event.is_keyboard() || ARM.with_borrow_mut(|arm| arm.up(name))
}

pub(crate) fn stop(el: Stateful<Div>, name: impl Into<SharedString>, on: bool) -> Stateful<Div> {
    answers(el.focusable().tab_stop(on), name.into())
}

pub(crate) fn stop_at(
    el: Stateful<Div>,
    handle: &FocusHandle,
    name: impl Into<SharedString>,
    on: bool,
) -> Stateful<Div> {
    answers(el.track_focus(&handle.clone().tab_stop(on)), name.into())
}

fn answers(el: Stateful<Div>, name: SharedString) -> Stateful<Div> {
    el.focus(|s| s.shadow(glass::ring(colours::brand())))
        .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            match key_of(&event.keystroke.key, event.keystroke.modifiers) {
                Key::Press => ARM.with_borrow_mut(|arm| arm.down(&name, event.is_held)),
                Key::Next => window.focus_next(),
                Key::Previous => window.focus_prev(),
                Key::Other => return,
            }
            cx.stop_propagation();
        })
}

#[cfg(test)]
mod tests;
