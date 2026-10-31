use std::ops::Range;

use gpui::{
    App, AvailableSpace, Bounds, ClipboardItem, Context, CursorStyle, ElementInputHandler, Entity, EntityInputHandler,
    EventEmitter, FocusHandle, Focusable, GlobalElementId, Hsla, KeyBinding, LayoutId, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, SharedString, Size, Style, TextAlign, TextRun,
    UTF16Selection, UnderlineStyle, Window, WrappedLine, actions, div, fill, point, prelude::*, px, relative, size,
};
use unicode_segmentation::UnicodeSegmentation;

actions!(
    text_field,
    [
        Submit,
        Cancel,
        DeleteWordLeft,
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
}

struct Layout {
    line: WrappedLine,
    line_height: Pixels,
    bounds: Bounds<Pixels>,
}

#[derive(Default)]
struct Options {
    masked: bool,
    clears_on_escape: bool,
    wraps: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum Escaped {
    Cancel,
    Clear,
    Revert,
    Pass,
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
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx);
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.content.len(), cx);
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

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &text.replace('\n', " "), window, cx);
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
        self.selected_range = offset..offset;
        cx.notify();
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed { self.selected_range.start } else { self.selected_range.end }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }

        let Some(layout) = self.last_layout.as_ref() else { return 0 };
        if position.y < layout.bounds.top() {
            return 0;
        }
        if position.y >= layout.bounds.bottom() {
            return self.content.len();
        }
        let local = position.relative_to(&layout.bounds.origin);
        layout.line.closest_index_for_position(local, layout.line_height).unwrap_or_else(|index| index)
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset;
        } else {
            self.selected_range.end = offset;
        }
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
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
        self.content.grapheme_indices(true).rev().find_map(|(idx, _)| (idx < offset).then_some(idx)).unwrap_or(0)
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
        let start = layout.line.position_for_index(range.start, layout.line_height)?;
        let end = layout.line.position_for_index(range.end, layout.line_height).filter(|end| end.y == start.y);
        let top = bounds.top() + start.y;
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
        let local = layout.bounds.localize(&point)?;
        let utf8_index = layout.line.index_for_position(local, layout.line_height).ok()?;
        Some(self.offset_to_utf16(utf8_index))
    }
}

struct TextElement {
    input: Entity<TextField>,
}

#[derive(Default)]
struct PrepaintState {
    line: Option<WrappedLine>,
    cursor: Option<PaintQuad>,
    selection: Vec<PaintQuad>,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    super::anonymous_element!();

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        let line_height = window.line_height();
        if !self.input.read(cx).options.wraps {
            style.size.height = line_height.into();
            return (window.request_layout(style, [], cx), ());
        }
        let (text, runs, font_size) = self.shaping(window, cx);
        let layout = window.request_measured_layout(style, move |known, available, window, _| {
            let width = known.width.or(match available.width {
                AvailableSpace::Definite(width) => Some(width),
                _ => None,
            });
            let rows = window
                .text_system()
                .shape_text(text.clone(), font_size, &runs, width, None)
                .map_or(1, |lines| lines.iter().map(|line| line.wrap_boundaries().len() + 1).sum());
            Size { width: width.unwrap_or_default(), height: line_height * rows }
        });
        (layout, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let (text, runs, font_size) = self.shaping(window, cx);
        let input = self.input.read(cx);
        let wrap_width = input.options.wraps.then_some(bounds.size.width);
        let shaped = window.text_system().shape_text(text, font_size, &runs, wrap_width, None);
        let Some(line) = shaped.ok().and_then(|lines| lines.into_iter().next()) else {
            return PrepaintState::default();
        };
        let at = Place { bounds, line_height: window.line_height() };
        let selected = &input.selected_range;
        let (selection, cursor) = if selected.is_empty() {
            (Vec::new(), at.caret(&line, input.cursor_offset()))
        } else {
            (at.selection(&line, selected), None)
        };
        PrepaintState { line: Some(line), cursor, selection }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(&focus_handle, ElementInputHandler::new(bounds, self.input.clone()), cx);
        for selection in prepaint.selection.drain(..) {
            window.paint_quad(selection);
        }
        let Some(line) = prepaint.line.take() else { return };
        let line_height = window.line_height();
        let _ = line.paint(bounds.origin, line_height, TextAlign::Left, None, window, cx);

        if focus_handle.is_focused(window)
            && let Some(cursor) = prepaint.cursor.take()
        {
            window.paint_quad(cursor);
        }

        self.input.update(cx, |input, _cx| input.last_layout = Some(Layout { line, line_height, bounds }));
    }
}

impl TextElement {
    fn shaping(&self, window: &Window, cx: &App) -> (SharedString, Vec<TextRun>, Pixels) {
        let input = self.input.read(cx);
        let style = window.text_style();
        let (display_text, text_color) = input.display_text(style.color);
        let run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = text_runs(run, input.marked_range.as_ref());
        (display_text, runs, style.font_size.to_pixels(window.rem_size()))
    }
}

impl TextField {
    fn display_text(&self, colour: Hsla) -> (SharedString, Hsla) {
        if self.content.is_empty() {
            return (self.placeholder.clone(), crate::theme::colours::muted().into());
        }
        let shown =
            if self.options.masked { "•".repeat(self.content.chars().count()).into() } else { self.content.clone() };
        (shown, colour)
    }
}

fn text_runs(run: TextRun, marked: Option<&Range<usize>>) -> Vec<TextRun> {
    let Some(marked) = marked else { return vec![run] };
    let total = run.len;
    vec![
        TextRun { len: marked.start, ..run.clone() },
        TextRun {
            len: marked.end - marked.start,
            underline: Some(UnderlineStyle { color: Some(run.color), thickness: px(1.0), wavy: false }),
            ..run.clone()
        },
        TextRun { len: total - marked.end, ..run },
    ]
    .into_iter()
    .filter(|run| run.len > 0)
    .collect()
}

struct Place {
    bounds: Bounds<Pixels>,
    line_height: Pixels,
}

impl Place {
    fn caret(&self, line: &WrappedLine, cursor: usize) -> Option<PaintQuad> {
        let at = line.position_for_index(cursor, self.line_height)?;
        let area = Bounds::new(self.bounds.origin + at, size(px(2.), self.line_height));
        Some(fill(area, crate::theme::colours::accent()))
    }

    fn selection(&self, line: &WrappedLine, selected: &Range<usize>) -> Vec<PaintQuad> {
        let layout = &line.unwrapped_layout;
        let starts = row_starts(line);
        selected_rows(&starts, line.len(), selected)
            .into_iter()
            .map(|(row, range)| {
                let x = |index| self.bounds.left() + layout.x_for_index(index) - layout.x_for_index(starts[row]);
                let top = self.bounds.top() + self.line_height * row;
                let area =
                    Bounds::from_corners(point(x(range.start), top), point(x(range.end), top + self.line_height));
                fill(area, crate::theme::colours::selection())
            })
            .collect()
    }
}

fn row_starts(line: &WrappedLine) -> Vec<usize> {
    let runs = &line.unwrapped_layout.runs;
    let wraps = line.wrap_boundaries().iter().map(|wrap| runs[wrap.run_ix].glyphs[wrap.glyph_ix].index);
    std::iter::once(0).chain(wraps).collect()
}

fn selected_rows(starts: &[usize], len: usize, selected: &Range<usize>) -> Vec<(usize, Range<usize>)> {
    let ends = starts.iter().skip(1).copied().chain([len]);
    starts
        .iter()
        .zip(ends)
        .enumerate()
        .filter_map(|(row, (&start, end))| {
            let range = selected.start.max(start)..selected.end.min(end);
            (range.start < range.end).then_some((row, range))
        })
        .collect()
}

impl Render for TextField {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .key_context("TextField")
            .track_focus(&self.focus_handle(cx))
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::delete_word_left))
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
    let ctx = Some("TextField");
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, ctx),
        KeyBinding::new("delete", Delete, ctx),
        KeyBinding::new("alt-backspace", DeleteWordLeft, ctx),
        KeyBinding::new("cmd-backspace", DeleteWordLeft, ctx),
        KeyBinding::new("left", Left, ctx),
        KeyBinding::new("right", Right, ctx),
        KeyBinding::new("shift-left", SelectLeft, ctx),
        KeyBinding::new("shift-right", SelectRight, ctx),
        KeyBinding::new("cmd-a", SelectAll, ctx),
        KeyBinding::new("cmd-v", Paste, ctx),
        KeyBinding::new("cmd-c", Copy, ctx),
        KeyBinding::new("cmd-x", Cut, ctx),
        KeyBinding::new("home", Home, ctx),
        KeyBinding::new("cmd-left", Home, ctx),
        KeyBinding::new("end", End, ctx),
        KeyBinding::new("cmd-right", End, ctx),
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, ctx),
        KeyBinding::new("enter", Submit, ctx),
        KeyBinding::new("escape", Cancel, ctx),
        KeyBinding::new("tab", FocusNext, ctx),
        KeyBinding::new("shift-tab", FocusPrevious, ctx),
    ]);
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
        }
    }

    pub(crate) fn masked(mut self) -> Self {
        self.options.masked = true;
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

    pub(crate) fn reverts_on_escape(mut self) -> Self {
        self.saved = Some(SharedString::default());
        self
    }

    pub(crate) fn edited(&self) -> bool {
        self.saved.as_ref().is_some_and(|saved| *saved != self.content)
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
        let end = self.content.len();
        self.selected_range = end..end;
        self.marked_range = None;
        cx.emit(TextFieldEvent::Changed);
        cx.notify();
    }

    pub(crate) fn clear(&mut self, cx: &mut Context<Self>) {
        self.set_text("", cx);
    }

    fn delete_word_left(&mut self, _: &DeleteWordLeft, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let before = &self.content[..self.cursor_offset()];
            let trimmed = before.trim_end();
            let start = trimmed.rfind(char::is_whitespace).map_or(0, |i| i + 1);
            self.select_to(start, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }
}

#[cfg(test)]
mod tests;
