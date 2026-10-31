use std::ops::Range;

use gpui::{
    Bounds, Context, EntityInputHandler, KeyDownEvent, KeyUpEvent, Keystroke, Pixels, Point, UTF16Selection, Window,
    px, size,
};
use serde_json::{Value, json};

use super::Browser;
use super::keyboard::{self, Route};
use super::viewport::{Rect, from_page};
use crate::keymap;

pub(crate) type Outgoing = (&'static str, Value);

#[derive(Default)]
pub(crate) struct Typing {
    pending: Option<Keystroke>,
    marked: String,
    pub(crate) caret: Option<Rect>,
}

fn utf16(text: &str) -> usize {
    text.encode_utf16().count()
}

impl Typing {
    pub(crate) fn pressed(&mut self, key: &Keystroke) {
        self.pending = Some(key.clone());
    }

    pub(crate) fn commit(&mut self, text: &str) -> Option<Outgoing> {
        let composing = !std::mem::take(&mut self.marked).is_empty();
        let key = self.pending.take().filter(|_| !composing);
        let text: String = text.chars().filter(|c| !c.is_control()).collect();
        match key {
            Some(key) if !text.is_empty() => Some(("Input.dispatchKeyEvent", keyboard::typed(&key, &text, false))),
            _ if text.is_empty() && !composing => None,
            _ => Some(("Input.insertText", json!({ "text": text }))),
        }
    }

    pub(crate) fn compose(&mut self, text: &str, selected: Option<Range<usize>>) -> Outgoing {
        self.pending = None;
        self.marked = text.to_string();
        let end = utf16(text);
        let selected = selected.unwrap_or(end..end);
        let params = json!({ "text": text, "selectionStart": selected.start, "selectionEnd": selected.end });
        ("Input.imeSetComposition", params)
    }

    pub(crate) fn unmark(&mut self) -> Option<Outgoing> {
        let marked = std::mem::take(&mut self.marked);
        (!marked.is_empty()).then(|| ("Input.insertText", json!({ "text": marked })))
    }

    fn marked_range(&self) -> Option<Range<usize>> {
        (!self.marked.is_empty()).then(|| 0..utf16(&self.marked))
    }
}

pub(crate) fn window_claims(key: &Keystroke) -> bool {
    key.modifiers.platform && keymap::find(&key.key, key.modifiers.shift, true).is_some()
}

impl Browser {
    pub(super) fn tell_page(&self, (method, params): Outgoing) {
        if let Some(session) = self.active_session() {
            self.send(Some(&session), method, params);
        }
    }

    pub(super) fn page_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        if key.key == "escape" || window_claims(key) {
            return;
        }
        if self.input.choice.is_some() {
            self.choice_key(key, cx);
            cx.stop_propagation();
            return;
        }
        match keyboard::route(key, event.is_held) {
            Route::Text => self.input.typing.pressed(key),
            Route::Repeat(text) => {
                self.tell_page(("Input.dispatchKeyEvent", keyboard::typed(key, &text, true)));
                cx.stop_propagation();
            }
            Route::Key => {
                self.tell_page(("Input.dispatchKeyEvent", keyboard::key_down(key, event.is_held)));
                cx.stop_propagation();
            }
        }
    }

    pub(super) fn page_key_up(&mut self, event: &KeyUpEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        if window_claims(key) || self.input.choice.is_some() {
            return;
        }
        self.tell_page(("Input.dispatchKeyEvent", keyboard::key_up(key)));
        cx.stop_propagation();
    }

    pub(super) fn press_escape(&self) {
        let escape = Keystroke { key: "escape".into(), ..Keystroke::default() };
        self.tell_page(("Input.dispatchKeyEvent", keyboard::key_down(&escape, false)));
    }

    fn caret_bounds(&self, element: Bounds<Pixels>) -> Bounds<Pixels> {
        let meta = self.tabs.active(self.workspace).and_then(|t| t.shot.as_ref()).map(|s| s.meta);
        match (self.input.typing.caret, meta) {
            (Some(caret), Some(meta)) => from_page(caret, element, &meta),
            _ => Bounds::new(element.origin, size(px(1.), px(18.))),
        }
    }
}

impl EntityInputHandler for Browser {
    fn character_index_for_point(&mut self, _: Point<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        None
    }

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        element: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(self.caret_bounds(element))
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        marked: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
        let outgoing = self.input.typing.compose(marked, selected);
        self.tell_page(outgoing);
    }

    fn replace_text_in_range(&mut self, _: Option<Range<usize>>, text: &str, _: &mut Window, _: &mut Context<Self>) {
        if let Some(outgoing) = self.input.typing.commit(text) {
            self.tell_page(outgoing);
        }
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        if let Some(outgoing) = self.input.typing.unmark() {
            self.tell_page(outgoing);
        }
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.input.typing.marked_range()
    }

    fn selected_text_range(&mut self, _: bool, _: &mut Window, _: &mut Context<Self>) -> Option<UTF16Selection> {
        let end = utf16(&self.input.typing.marked);
        Some(UTF16Selection { range: end..end, reversed: false })
    }

    fn text_for_range(
        &mut self,
        _: Range<usize>,
        _: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests;
