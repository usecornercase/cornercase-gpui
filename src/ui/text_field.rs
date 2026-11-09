use std::ops::{Range, RangeInclusive};
use std::rc::Rc;

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, Div, EntityInputHandler, EventEmitter, FocusHandle, Focusable,
    Hsla, KeyBinding, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollHandle,
    ScrollWheelEvent, SharedString, UTF16Selection, Window, actions, div, point, prelude::*, px,
};
use unicode_segmentation::UnicodeSegmentation;

use self::element::{Layout, TextElement};
use self::rows::{clamp_scroll, line_breaks, one_line, previous_grapheme, shown_without_controls};
use crate::desktop;
use crate::platform::Os;

mod element;
mod rows;

actions!(
    text_field,
    [
        Submit,
        Cancel,
        DeleteWordLeft,
        DeleteWordRight,
        WordLeft,
        WordRight,
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        ShowCharacterPalette,
        Paste,
        Cut,
        Copy,
        FocusNext,
        FocusPrevious,
        NewLine,
        Up,
        Down,
        SelectUp,
        SelectDown,
        SelectHome,
        SelectEnd,
        TextStart,
        TextEnd,
    ]
);

pub(crate) struct TextField {
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_layout: Option<Layout>,
    is_selecting: bool,
    options: Options,
    saved: Option<SharedString>,
    view: View,
}

const LINES_CONTEXT: &str = "TextField Multiline";

type DownHook = Rc<dyn Fn(&mut Window, &mut App) -> bool>;

#[derive(Default)]
struct Options {
    masked: bool,
    clears_on_escape: bool,
    wraps: bool,
    rows: Option<RangeInclusive<usize>>,
    scroller: Option<ScrollHandle>,
    on_down: Option<DownHook>,
}

#[derive(Default)]
struct View {
    scroll_top: Pixels,
    reveal: bool,
    was_focused: bool,
    goal_x: Option<Pixels>,
}

#[derive(Debug, PartialEq, Eq)]
enum Escaped {
    Cancel,
    Clear,
    Revert,
    Pass,
}

fn shared<'a>(content: &'a str, selected: &Range<usize>, masked: bool) -> Option<&'a str> {
    content.get(selected.clone()).filter(|text| !masked && !text.is_empty())
}

fn escaped(clears: bool, content: &str, saved: Option<&str>) -> Escaped {
    match saved {
        Some(saved) if content != saved => Escaped::Revert,
        Some(_) => Escaped::Pass,
        None if !clears => Escaped::Cancel,
        None if content.is_empty() => Escaped::Pass,
        None => Escaped::Clear,
    }
}

impl TextField {
    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx);
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx);
        }
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
        self.share_selection(cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
        self.share_selection(cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx);
        self.share_selection(cx);
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.row_ends().map_or(0, |(start, _)| start), cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.row_ends().map_or(self.content.len(), |(_, end)| end), cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn on_mouse_down(&mut self, event: &MouseDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        self.is_selecting = true;

        if event.modifiers.shift {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        } else {
            self.move_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.is_selecting) {
            self.share_selection(cx);
        }
    }

    fn share_selection(&self, cx: &App) {
        if let Some(text) = shared(&self.content, &self.selected_range, self.options.masked) {
            desktop::write_primary(text, cx);
        }
    }

    fn paste_primary(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = desktop::read_primary(cx) else { return };
        self.move_to(self.index_for_mouse_position(event.position), cx);
        self.insert_pasted(&text, window, cx);
    }

    fn insert_pasted(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let text = if self.options.rows.is_some() { line_breaks(text) } else { one_line(text) };
        self.replace_text_in_range(None, &text, window, cx);
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.insert_pasted(&text, window, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(self.content[self.selected_range.clone()].to_string()));
        }
    }
    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(self.content[self.selected_range.clone()].to_string()));
            self.replace_text_in_range(None, "", window, cx);
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        let offset = self.content.floor_char_boundary(offset);
        self.selected_range = offset..offset;
        self.caret_moved();
        cx.notify();
    }

    fn caret_moved(&mut self) {
        self.view.goal_x = None;
        self.view.reveal = true;
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed { self.selected_range.start } else { self.selected_range.end }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }
        self.last_layout.as_ref().map_or(0, |layout| layout.index_at(position, &self.content))
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        let offset = self.content.floor_char_boundary(offset);
        if self.selection_reversed {
            self.selected_range.start = offset;
        } else {
            self.selected_range.end = offset;
        }
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        self.caret_moved();
        cx.notify();
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;

        for ch in self.content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }

        utf8_offset
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;

        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }

        utf16_offset
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    fn splice(&mut self, range_utf16: Option<&Range<usize>>, new_text: &str) -> Range<usize> {
        let range = range_utf16
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        self.content = (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..]).into();
        range
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        previous_grapheme(&self.content, offset)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }
}

impl EntityInputHandler for TextField {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection { range: self.range_to_utf16(&self.selected_range), reversed: self.selection_reversed })
    }

    fn marked_text_range(&self, _window: &mut Window, _cx: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked_range.as_ref().map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = self.splice(range_utf16.as_ref(), new_text);
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.marked_range.take();
        self.caret_moved();
        cx.emit(TextFieldEvent::Changed);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = self.splice(range_utf16.as_ref(), new_text);
        if new_text.is_empty() {
            self.marked_range = None;
        } else {
            self.marked_range = Some(range.start..range.start + new_text.len());
        }
        self.selected_range =
            new_selected_range_utf16.as_ref().map(|range_utf16| self.range_from_utf16(range_utf16)).map_or_else(
                || range.start + new_text.len()..range.start + new_text.len(),
                |new_range| new_range.start + range.start..new_range.end + range.end,
            );
        self.caret_moved();
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let layout = self.last_layout.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        let start = layout.position(range.start);
        let end = Some(layout.position(range.end)).filter(|end| end.y == start.y);
        let top = bounds.top() + start.y - layout.scroll_top;
        Some(Bounds::from_corners(
            point(bounds.left() + start.x, top),
            point(bounds.left() + end.unwrap_or(start).x, top + layout.line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let layout = self.last_layout.as_ref()?;
        layout.bounds.localize(&point)?;
        Some(self.offset_to_utf16(layout.index_at(point, &self.content)))
    }
}

impl TextField {
    fn display_text(&self, colour: Hsla) -> (SharedString, Hsla) {
        if self.content.is_empty() {
            return (self.placeholder.clone(), crate::theme::colours::muted().into());
        }
        if self.options.masked {
            return ("•".repeat(self.content.chars().count()).into(), colour);
        }
        let shown = shown_without_controls(&self.content).map_or_else(|| self.content.clone(), SharedString::from);
        (shown, colour)
    }
}

impl Render for TextField {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root = if self.options.rows.is_some() {
            Self::line_keys(div().key_context(LINES_CONTEXT), cx)
        } else {
            div().key_context("TextField")
        };
        root.flex()
            .track_focus(&self.focus_handle(cx))
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::delete_word_left))
            .on_action(cx.listener(Self::delete_word_right))
            .on_action(cx.listener(Self::word_left))
            .on_action(cx.listener(Self::word_right))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(|_: &ShowCharacterPalette, window, _| window.show_character_palette())
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(|_, _: &Submit, _, cx| cx.emit(TextFieldEvent::Submit)))
            .on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(Self::focus_next))
            .on_action(cx.listener(Self::focus_previous))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .when(desktop::has_primary(Os::CURRENT), |d| {
                d.on_mouse_down(MouseButton::Middle, cx.listener(Self::paste_primary))
            })
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .w_full()
            .line_height(px(20.))
            .child(TextElement { input: cx.entity() })
    }
}

impl Focusable for TextField {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

pub(crate) enum TextFieldEvent {
    Changed,
    Submit,
    Cancel,
}

impl EventEmitter<TextFieldEvent> for TextField {}

pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys(bindings(Os::CURRENT));
}

fn bindings(os: Os) -> Vec<KeyBinding> {
    let ctx = Some("TextField");
    let mut keys = vec![
        KeyBinding::new("backspace", Backspace, ctx),
        KeyBinding::new("delete", Delete, ctx),
        KeyBinding::new("left", Left, ctx),
        KeyBinding::new("right", Right, ctx),
        KeyBinding::new("shift-left", SelectLeft, ctx),
        KeyBinding::new("shift-right", SelectRight, ctx),
        KeyBinding::new("home", Home, ctx),
        KeyBinding::new("end", End, ctx),
        KeyBinding::new("enter", Submit, ctx),
        KeyBinding::new("escape", Cancel, ctx),
        KeyBinding::new("tab", FocusNext, ctx),
        KeyBinding::new("shift-tab", FocusPrevious, ctx),
    ];
    keys.extend(match os {
        Os::Mac => mac_bindings(ctx),
        Os::Linux | Os::Windows => control_bindings(ctx),
    });
    keys.extend(line_bindings(os));
    keys
}

fn line_bindings(os: Os) -> Vec<KeyBinding> {
    let ctx = Some("Multiline");
    let mut keys = vec![
        KeyBinding::new("enter", NewLine, ctx),
        KeyBinding::new("shift-enter", NewLine, ctx),
        KeyBinding::new("up", Up, ctx),
        KeyBinding::new("down", Down, ctx),
        KeyBinding::new("shift-up", SelectUp, ctx),
        KeyBinding::new("shift-down", SelectDown, ctx),
        KeyBinding::new("shift-home", SelectHome, ctx),
        KeyBinding::new("shift-end", SelectEnd, ctx),
    ];
    keys.push(KeyBinding::new(send_key(os).0, Submit, ctx));
    keys.extend(match os {
        Os::Mac => [KeyBinding::new("cmd-up", TextStart, ctx), KeyBinding::new("cmd-down", TextEnd, ctx)],
        Os::Linux | Os::Windows => {
            [KeyBinding::new("ctrl-home", TextStart, ctx), KeyBinding::new("ctrl-end", TextEnd, ctx)]
        }
    });
    keys
}

const PROMPT_ROWS: RangeInclusive<usize> = 3..=12;

pub(crate) fn send_key(os: Os) -> (&'static str, &'static str) {
    match os {
        Os::Mac => ("cmd-enter", "⌘↩"),
        Os::Linux | Os::Windows => ("ctrl-enter", "Ctrl+Enter"),
    }
}

pub(crate) fn prompt_keys(os: Os, verb: &str) -> String {
    let new_line = match os {
        Os::Mac => "↩",
        Os::Linux | Os::Windows => "Enter",
    };
    format!("{new_line} for a new line, {} to {verb}", send_key(os).1)
}

fn mac_bindings(ctx: Option<&str>) -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("alt-backspace", DeleteWordLeft, ctx),
        KeyBinding::new("cmd-backspace", DeleteWordLeft, ctx),
        KeyBinding::new("cmd-a", SelectAll, ctx),
        KeyBinding::new("cmd-v", Paste, ctx),
        KeyBinding::new("cmd-c", Copy, ctx),
        KeyBinding::new("cmd-x", Cut, ctx),
        KeyBinding::new("cmd-left", Home, ctx),
        KeyBinding::new("cmd-right", End, ctx),
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, ctx),
    ]
}

fn control_bindings(ctx: Option<&str>) -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("ctrl-backspace", DeleteWordLeft, ctx),
        KeyBinding::new("ctrl-delete", DeleteWordRight, ctx),
        KeyBinding::new("ctrl-left", WordLeft, ctx),
        KeyBinding::new("ctrl-right", WordRight, ctx),
        KeyBinding::new("ctrl-a", SelectAll, ctx),
        KeyBinding::new("ctrl-v", Paste, ctx),
        KeyBinding::new("ctrl-c", Copy, ctx),
        KeyBinding::new("ctrl-x", Cut, ctx),
        KeyBinding::new("ctrl-shift-v", Paste, ctx),
        KeyBinding::new("ctrl-shift-c", Copy, ctx),
    ]
}

fn word_start(text: &str, at: usize) -> usize {
    let before = text[..at].trim_end();
    before.char_indices().rev().find(|(_, c)| c.is_whitespace()).map_or(0, |(i, c)| i + c.len_utf8())
}

fn word_end(text: &str, at: usize) -> usize {
    let after = &text[at..];
    let lead = after.len() - after.trim_start().len();
    after[lead..].find(char::is_whitespace).map_or(text.len(), |i| at + lead + i)
}

impl TextField {
    pub(crate) fn new(placeholder: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: SharedString::default(),
            placeholder: placeholder.into(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            is_selecting: false,
            options: Options::default(),
            saved: None,
            view: View::default(),
        }
    }

    pub(crate) fn masked(mut self) -> Self {
        self.options.masked = true;
        self
    }

    pub(crate) fn on_down(mut self, hook: impl Fn(&mut Window, &mut App) -> bool + 'static) -> Self {
        self.options.on_down = Some(Rc::new(hook));
        self
    }

    pub(crate) fn in_tab_order(mut self) -> Self {
        self.focus_handle = self.focus_handle.tab_stop(true);
        self
    }

    pub(crate) fn clears_on_escape(mut self) -> Self {
        self.options.clears_on_escape = true;
        self
    }

    pub(crate) fn wraps(mut self) -> Self {
        self.options.wraps = true;
        self
    }

    pub(crate) fn multiline(mut self, rows: RangeInclusive<usize>) -> Self {
        self.options.wraps = true;
        self.options.rows = Some(rows);
        self
    }

    pub(crate) fn prompt(self) -> Self {
        self.multiline(PROMPT_ROWS)
    }

    pub(crate) fn inside(mut self, scroller: ScrollHandle) -> Self {
        self.options.scroller = Some(scroller);
        self
    }

    pub(crate) fn reverts_on_escape(mut self) -> Self {
        self.saved = Some(SharedString::default());
        self
    }

    pub(crate) fn edited(&self) -> bool {
        self.saved.as_ref().is_some_and(|saved| *saved != self.content)
    }

    pub(crate) fn set_placeholder(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        let text = text.into();
        if self.placeholder != text {
            self.placeholder = text;
            cx.notify();
        }
    }

    pub(crate) fn keep_saved(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        let text = text.into();
        let follow = !self.edited();
        self.saved = Some(text.clone());
        if follow && self.content != text {
            self.set_text(text, cx);
        }
    }

    pub(crate) fn revert(&mut self, cx: &mut Context<Self>) {
        if let Some(saved) = self.saved.clone() {
            self.set_text(saved, cx);
        }
    }

    fn cancel(&mut self, _: &Cancel, _: &mut Window, cx: &mut Context<Self>) {
        match escaped(self.options.clears_on_escape, &self.content, self.saved.as_ref().map(SharedString::as_ref)) {
            Escaped::Cancel => cx.emit(TextFieldEvent::Cancel),
            Escaped::Clear => self.clear(cx),
            Escaped::Pass => cx.propagate(),
            Escaped::Revert => {
                self.revert(cx);
                cx.emit(TextFieldEvent::Cancel);
            }
        }
    }

    fn focus_next(&mut self, _: &FocusNext, window: &mut Window, cx: &mut Context<Self>) {
        if self.focus_handle.tab_stop { window.focus_next() } else { cx.propagate() }
    }

    fn focus_previous(&mut self, _: &FocusPrevious, window: &mut Window, cx: &mut Context<Self>) {
        if self.focus_handle.tab_stop { window.focus_prev() } else { cx.propagate() }
    }

    pub(crate) fn text(&self) -> String {
        self.content.to_string()
    }

    pub(crate) fn set_text(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.content = text.into();
        let caret = if self.options.rows.is_some() { 0 } else { self.content.len() };
        self.selected_range = caret..caret;
        self.marked_range = None;
        self.view.scroll_top = px(0.);
        cx.emit(TextFieldEvent::Changed);
        cx.notify();
    }

    pub(crate) fn clear(&mut self, cx: &mut Context<Self>) {
        self.set_text("", cx);
    }

    fn delete_word_left(&mut self, _: &DeleteWordLeft, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(word_start(&self.content, self.cursor_offset()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete_word_right(&mut self, _: &DeleteWordRight, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(word_end(&self.content, self.cursor_offset()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(word_start(&self.content, self.cursor_offset()), cx);
    }

    fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(word_end(&self.content, self.cursor_offset()), cx);
    }
}

impl TextField {
    fn line_keys(root: Div, cx: &mut Context<Self>) -> Div {
        root.on_action(cx.listener(|this, _: &NewLine, window, cx| this.replace_text_in_range(None, "\n", window, cx)))
            .on_action(cx.listener(|this, _: &Up, _, cx| this.step_row(true, false, cx)))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(|this, _: &SelectUp, _, cx| this.step_row(true, true, cx)))
            .on_action(cx.listener(|this, _: &SelectDown, _, cx| this.step_row(false, true, cx)))
            .on_action(cx.listener(Self::select_home))
            .on_action(cx.listener(Self::select_end))
            .on_action(cx.listener(|this, _: &TextStart, _, cx| this.move_to(0, cx)))
            .on_action(cx.listener(|this, _: &TextEnd, _, cx| this.move_to(this.content.len(), cx)))
            .on_scroll_wheel(cx.listener(Self::on_wheel))
    }

    fn select_home(&mut self, _: &SelectHome, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.row_ends().map_or(0, |(start, _)| start), cx);
        self.share_selection(cx);
    }

    fn select_end(&mut self, _: &SelectEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.row_ends().map_or(self.content.len(), |(_, end)| end), cx);
        self.share_selection(cx);
    }

    fn row_ends(&self) -> Option<(usize, usize)> {
        self.options.rows.as_ref()?;
        let layout = self.last_layout.as_ref().filter(|_| !self.content.is_empty())?;
        let row = layout.rows.get(layout.rows.row_of(self.cursor_offset()))?;
        Some((row.start, row.last_caret(&self.content)))
    }

    fn down(&mut self, _: &Down, window: &mut Window, cx: &mut Context<Self>) {
        let taken = self.options.on_down.clone().is_some_and(|hook| hook(window, cx));
        if !taken {
            self.step_row(false, false, cx);
        }
    }

    fn step_row(&mut self, up: bool, select: bool, cx: &mut Context<Self>) {
        let Some((target, x)) = self.row_step(up) else { return };
        if select {
            self.select_to(target, cx);
            self.share_selection(cx);
        } else {
            self.move_to(target, cx);
        }
        self.view.goal_x = Some(x);
    }

    fn row_step(&self, up: bool) -> Option<(usize, Pixels)> {
        let layout = self.last_layout.as_ref().filter(|_| !self.content.is_empty())?;
        let cursor = self.cursor_offset();
        let x = self.view.goal_x.unwrap_or_else(|| layout.position(cursor).x);
        let here = layout.rows.row_of(cursor);
        let row = if up { here.checked_sub(1).and_then(|row| layout.rows.get(row)) } else { layout.rows.get(here + 1) };
        let target = match row {
            Some(row) => layout.index_in_row(row, x, &self.content),
            None if up => 0,
            None => self.content.len(),
        };
        Some((target, x))
    }

    fn on_wheel(&mut self, event: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(layout) = self.last_layout.as_ref() else { return };
        let delta = event.delta.pixel_delta(layout.line_height).y;
        let next = clamp_scroll(self.view.scroll_top - delta, layout.content_height(), layout.bounds.size.height);
        if next != self.view.scroll_top {
            self.view.scroll_top = next;
            cx.stop_propagation();
            cx.notify();
        }
    }
}

#[cfg(test)]
mod tests;
