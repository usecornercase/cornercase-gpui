use std::ops::Range;

use gpui::{
    App, AvailableSpace, Bounds, ContentMask, DispatchPhase, Element, Entity, GlobalElementId, IntoElement, LayoutId,
    MouseMoveEvent, PaintQuad, Pixels, Point, ScrollHandle, SharedString, Size, Style, TextAlign, TextRun, Window,
    WrappedLine, fill, point, px, relative, size,
};

use super::TextField;
use super::handler::Handler;
use super::rows::{
    CARET_WIDTH, Row, Rows, clamp_scroll, measured_rows, rows_shown, scroll_from_top, scroll_to_show,
    scroll_to_show_with_margins, sideways, thumb,
};
use super::shown::Shown;
use crate::theme::colours;
use crate::ui::selection::BREAK_WIDTH;

const THUMB_WIDTH: Pixels = px(3.);
const THUMB_GAP: Pixels = px(4.);
const SCROLLER_MARGIN: Pixels = px(12.);
const PROMPT_MARGIN: Pixels = px(32.);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Head {
    #[default]
    No,
    Opening,
    Yes,
}

impl Head {
    fn painted(self) -> Self {
        if self == Self::Opening { Self::Yes } else { self }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Below {
    #[default]
    Margin,
    ToEnd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reveal {
    Still,
    Caret,
    Form,
}

fn reveal_of(moved: bool, first_focus: bool, head: Head) -> Reveal {
    match (moved, first_focus, head) {
        (true, _, _) | (false, true, Head::No) => Reveal::Caret,
        (false, true, Head::Yes) => Reveal::Form,
        (false, true, Head::Opening) | (false, false, _) => Reveal::Still,
    }
}

fn room_below(below: Below, prompt: bool, to_end: Pixels) -> Pixels {
    let margin = if prompt { PROMPT_MARGIN } else { SCROLLER_MARGIN };
    match below {
        Below::Margin => margin,
        Below::ToEnd => to_end.max(margin),
    }
}

fn next_scroll(
    why: Reveal,
    scroll: Pixels,
    caret: (Pixels, Pixels),
    margins: (Pixels, Pixels),
    height: Pixels,
) -> Pixels {
    let next = match why {
        Reveal::Still => scroll,
        Reveal::Caret => scroll_to_show_with_margins(scroll, caret, margins, height),
        Reveal::Form => scroll_from_top(scroll, caret, margins, height),
    };
    next.max(px(0.))
}

pub(super) struct Layout {
    lines: Vec<WrappedLine>,
    rows: Rows,
    shown: Shown,
    pub(super) line_height: Pixels,
    pub(super) bounds: Bounds<Pixels>,
    pub(super) scroll_top: Pixels,
    pub(super) scroll_left: Pixels,
}

impl Layout {
    fn row_at(&self, offset: usize) -> usize {
        self.rows.row_of(self.shown.to_shown(offset))
    }

    pub(super) fn position(&self, offset: usize) -> Point<Pixels> {
        let at = self.row_at(offset);
        let Some(row) = self.rows.get(at) else { return Point::default() };
        point(self.x_in_row(row, self.shown.to_shown(offset)), self.line_height * at)
    }

    fn x_in_row(&self, row: Row, index: usize) -> Pixels {
        let layout = &self.lines[row.line].unwrapped_layout;
        layout.x_for_index(index - row.line_start) - layout.x_for_index(row.start - row.line_start)
    }

    fn index_in_row(&self, row: Row, x: Pixels, text: &str) -> usize {
        let layout = &self.lines[row.line].unwrapped_layout;
        let index = layout.closest_index_for_x(x + layout.x_for_index(row.start - row.line_start));
        let (start, last) = self.shown.row_ends(row, text);
        self.shown.to_content(row.line_start + index).clamp(start, last)
    }

    pub(super) fn row_ends(&self, offset: usize, text: &str) -> Option<(usize, usize)> {
        let row = self.rows.get(self.row_at(offset))?;
        Some(self.shown.row_ends(row, text))
    }

    pub(super) fn index_beside(&self, offset: usize, up: bool, x: Pixels, text: &str) -> Option<usize> {
        let here = self.row_at(offset);
        let row = if up { here.checked_sub(1).and_then(|row| self.rows.get(row)) } else { self.rows.get(here + 1) };
        Some(self.index_in_row(row?, x, text))
    }

    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "a row's number from its height")]
    pub(super) fn index_at(&self, position: Point<Pixels>, text: &str) -> usize {
        let y = position.y - self.bounds.top() + self.scroll_top;
        if y < px(0.) {
            return 0;
        }
        match self.rows.get((y / self.line_height) as usize) {
            Some(row) => self.index_in_row(row, position.x - self.text_left(), text),
            None => text.len(),
        }
    }

    pub(super) fn content_height(&self) -> Pixels {
        self.line_height * self.rows.len()
    }

    fn text_top(&self) -> Pixels {
        self.bounds.top() - self.scroll_top
    }

    fn caret(&self, offset: usize) -> PaintQuad {
        let at = self.position(offset);
        let origin = point(self.text_left() + at.x, self.text_top() + at.y);
        fill(Bounds::new(origin, size(CARET_WIDTH, self.line_height)), colours::accent())
    }

    fn selection(&self, selected: &Range<usize>) -> Vec<PaintQuad> {
        self.rows
            .selected(&self.shown.range_to_shown(selected))
            .into_iter()
            .filter_map(|span| {
                let row = self.rows.get(span.row)?;
                let x = |index| self.text_left() + self.x_in_row(row, index);
                let top = self.text_top() + self.line_height * span.row;
                let tail = if span.line_break { BREAK_WIDTH } else { px(0.) };
                let end = point(x(span.range.end) + tail, top + self.line_height);
                Some(fill(Bounds::from_corners(point(x(span.range.start), top), end), colours::selection()))
            })
            .collect()
    }

    fn scrollbar(&self) -> Option<PaintQuad> {
        let height = self.bounds.size.height;
        let (top, length) = thumb(self.scroll_top, self.content_height(), height)?;
        let origin = point(self.bounds.right() + THUMB_GAP, self.bounds.top() + top);
        Some(fill(Bounds::new(origin, size(THUMB_WIDTH, length)), colours::faint()).corner_radii(THUMB_WIDTH / 2.))
    }

    fn clip(&self) -> ContentMask<Pixels> {
        let origin = point(self.bounds.left() - CARET_WIDTH, self.bounds.top());
        let area = size(self.bounds.size.width + CARET_WIDTH * 2., self.bounds.size.height);
        ContentMask { bounds: Bounds::new(origin, area) }
    }

    fn text_width(&self) -> Pixels {
        self.lines.first().map_or(px(0.), |line| line.unwrapped_layout.width)
    }

    fn text_left(&self) -> Pixels {
        self.bounds.left() - self.scroll_left
    }

    fn mask(&self, multi_row: bool) -> ContentMask<Pixels> {
        if multi_row { self.clip() } else { ContentMask { bounds: self.bounds } }
    }

    fn paint_lines(&self, window: &mut Window, cx: &mut App) {
        let mut top = self.text_top();
        for line in &self.lines {
            let height = self.line_height * (line.wrap_boundaries().len() + 1);
            if top + height > self.bounds.top() && top < self.bounds.bottom() {
                let origin = point(self.text_left(), top);
                let _ = line.paint(origin, self.line_height, TextAlign::Left, None, window, cx);
            }
            top += height;
        }
    }
}

pub(super) struct TextElement {
    pub(super) input: Entity<TextField>,
}

#[derive(Default)]
pub(super) struct PrepaintState {
    layout: Option<Layout>,
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

    crate::ui::anonymous_element!();

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
        let options = &self.input.read(cx).options;
        if !options.wraps {
            style.size.height = line_height.into();
            return (window.request_layout(style, [], cx), ());
        }
        let limits = options.rows.clone();
        let empty = self.input.read(cx).content.is_empty();
        let (text, runs, font_size, _) = self.shaping(window, cx);
        let layout = window.request_measured_layout(style, move |known, available, window, _| {
            let width = known.width.or(match available.width {
                AvailableSpace::Definite(width) => Some(width),
                _ => None,
            });
            let shaped = window
                .text_system()
                .shape_text(text.clone(), font_size, &runs, width, None)
                .map_or(1, |lines| lines.iter().map(|line| line.wrap_boundaries().len() + 1).sum());
            let rows = measured_rows(empty, shaped);
            Size { width: width.unwrap_or_default(), height: line_height * rows_shown(rows, limits.as_ref()) }
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
        let Some(layout) = self.layout(bounds, window, cx) else { return PrepaintState::default() };
        let layout = self.settled(layout, window, cx);
        let input = self.input.read(cx);
        let selected = &input.selected_range;
        let (selection, cursor) = if selected.is_empty() {
            (Vec::new(), Some(layout.caret(input.cursor_offset())))
        } else {
            (layout.selection(selected), None)
        };
        PrepaintState { layout: Some(layout), cursor, selection }
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
        let input = self.input.read(cx);
        let focus_handle = input.focus_handle.clone();
        let scrolls = input.options.rows.is_some() || !input.options.wraps;
        let multi_row = input.options.rows.is_some();
        let masked = input.options.masked;
        window.handle_input(&focus_handle, Handler::new(bounds, self.input.clone(), masked), cx);
        if scrolls {
            self.follow_drag(window);
        }
        let Some(layout) = prepaint.layout.take() else { return };
        let selection = std::mem::take(&mut prepaint.selection);
        let cursor = prepaint.cursor.take();
        let focused = focus_handle.is_focused(window);
        window.with_content_mask(scrolls.then(|| layout.mask(multi_row)), |window| {
            selection.into_iter().filter(|_| focused).for_each(|quad| window.paint_quad(quad));
            layout.paint_lines(window, cx);
            cursor.into_iter().filter(|_| focused).for_each(|quad| window.paint_quad(quad));
        });
        layout.scrollbar().into_iter().for_each(|quad| window.paint_quad(quad));
        self.input.update(cx, |input, _cx| input.last_layout = Some(layout));
    }
}

impl TextElement {
    fn follow_drag(&self, window: &mut Window) {
        let input = self.input.clone();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
            if phase == DispatchPhase::Bubble && event.dragging() && input.read(cx).is_selecting {
                input.update(cx, |input, cx| input.select_to(input.index_for_mouse_position(event.position), cx));
            }
        });
    }

    fn layout(&self, bounds: Bounds<Pixels>, window: &Window, cx: &App) -> Option<Layout> {
        let (text, runs, font_size, shown) = self.shaping(window, cx);
        let input = self.input.read(cx);
        let wraps = input.options.wraps;
        let wrap_width = (wraps && !input.content.is_empty()).then_some(bounds.size.width);
        let mut lines = window.text_system().shape_text(text, font_size, &runs, wrap_width, None).ok()?.into_vec();
        if !wraps {
            lines.truncate(1);
        }
        let rows = Rows::new(lines.iter().map(|line| (line.len(), row_starts(line))));
        let line_height = window.line_height();
        Some(Layout { lines, rows, shown, line_height, bounds, scroll_top: input.view.scroll_top, scroll_left: px(0.) })
    }

    fn settled(&self, mut layout: Layout, window: &Window, cx: &mut App) -> Layout {
        let input = self.input.read(cx);
        let focused = input.focus_handle.is_focused(window);
        let first_focus = focused && !input.view.was_focused;
        let reveal = input.view.reveal || first_focus;
        let caret = layout.position(input.cursor_offset()).y;
        let height = layout.bounds.size.height;
        let shown = if reveal {
            scroll_to_show(input.view.scroll_top, caret, caret + layout.line_height, height)
        } else {
            input.view.scroll_top
        };
        layout.scroll_top = clamp_scroll(shown, layout.content_height(), height);
        if focused && !input.options.wraps {
            let caret = layout.position(input.cursor_offset()).x;
            layout.scroll_left =
                sideways(input.view.scroll_left, (caret, layout.text_width()), layout.bounds.size.width);
        }
        let why = reveal_of(input.view.reveal, first_focus, input.view.head);
        if let Some(scroller) = input.options.scroller.as_ref().filter(|_| focused && why != Reveal::Still) {
            let shape = (why, input.options.below, input.options.rows.is_some());
            show_in_scroller(scroller, (layout.text_top() + caret, layout.line_height), shape, window);
        }
        let (scroll_top, scroll_left) = (layout.scroll_top, layout.scroll_left);
        self.input.update(cx, |input, _| {
            input.view.scroll_top = scroll_top;
            input.view.scroll_left = scroll_left;
            input.view.reveal = false;
            input.view.was_focused = focused;
            input.view.head = input.view.head.painted();
        });
        layout
    }

    fn shaping(&self, window: &Window, cx: &App) -> (SharedString, Vec<TextRun>, Pixels, Shown) {
        let input = self.input.read(cx);
        let style = window.text_style();
        let (display_text, text_color, shown) = input.display_text(style.color);
        let run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = shown.runs(run, input.marked_range.as_ref());
        (display_text, runs, style.font_size.to_pixels(window.rem_size()), shown)
    }
}

fn show_in_scroller(
    scroller: &ScrollHandle,
    (caret, line_height): (Pixels, Pixels),
    (why, below, prompt): (Reveal, Below, bool),
    window: &Window,
) {
    let view = scroller.bounds();
    let offset = scroller.offset();
    let scroll = -offset.y;
    let top = caret - view.top() + scroll;
    let to_end = view.size.height + scroller.max_offset().height - top - line_height;
    let margins = (SCROLLER_MARGIN, room_below(below, prompt, to_end));
    let next = next_scroll(why, scroll, (top, line_height), margins, view.size.height);
    if next != scroll {
        scroller.set_offset(point(offset.x, -next));
        window.request_animation_frame();
    }
}

fn row_starts(line: &WrappedLine) -> Vec<usize> {
    let runs = &line.unwrapped_layout.runs;
    let wraps = line.wrap_boundaries().iter().map(|wrap| runs[wrap.run_ix].glyphs[wrap.glyph_ix].index);
    std::iter::once(0).chain(wraps).collect()
}

#[cfg(test)]
mod tests;
