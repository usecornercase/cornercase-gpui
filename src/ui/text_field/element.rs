use std::ops::Range;

use gpui::{
    App, AvailableSpace, Bounds, ContentMask, DispatchPhase, Element, ElementInputHandler, Entity, GlobalElementId,
    IntoElement, LayoutId, MouseMoveEvent, PaintQuad, Pixels, Point, ScrollHandle, SharedString, Size, Style,
    TextAlign, TextRun, UnderlineStyle, Window, WrappedLine, fill, point, px, relative, size,
};

use super::TextField;
use super::rows::{
    CARET_WIDTH, Row, Rows, clamp_scroll, rows_shown, scroll_to_show, scroll_to_show_with_margins, sideways, thumb,
};
use crate::theme::colours;

const BREAK_WIDTH: Pixels = px(6.);
const THUMB_WIDTH: Pixels = px(3.);
const THUMB_GAP: Pixels = px(4.);
const SCROLLER_MARGIN: Pixels = px(12.);
const PROMPT_MARGIN: Pixels = px(32.);

pub(super) struct Layout {
    lines: Vec<WrappedLine>,
    pub(super) rows: Rows,
    pub(super) line_height: Pixels,
    pub(super) bounds: Bounds<Pixels>,
    pub(super) scroll_top: Pixels,
    pub(super) scroll_left: Pixels,
}

impl Layout {
    pub(super) fn position(&self, offset: usize) -> Point<Pixels> {
        let at = self.rows.row_of(offset);
        let Some(row) = self.rows.get(at) else { return Point::default() };
        point(self.x_in_row(row, offset), self.line_height * at)
    }

    fn x_in_row(&self, row: Row, offset: usize) -> Pixels {
        let layout = &self.lines[row.line].unwrapped_layout;
        layout.x_for_index(offset - row.line_start) - layout.x_for_index(row.start - row.line_start)
    }

    pub(super) fn index_in_row(&self, row: Row, x: Pixels, text: &str) -> usize {
        let layout = &self.lines[row.line].unwrapped_layout;
        let index = layout.closest_index_for_x(x + layout.x_for_index(row.start - row.line_start));
        (row.line_start + index).clamp(row.start, row.last_caret(text))
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
            .selected(selected)
            .into_iter()
            .filter_map(|span| {
                let row = self.rows.get(span.row)?;
                let x = |offset| self.text_left() + self.x_in_row(row, offset);
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
        window.handle_input(&focus_handle, ElementInputHandler::new(bounds, self.input.clone()), cx);
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
        let (text, runs, font_size) = self.shaping(window, cx);
        let input = self.input.read(cx);
        let wraps = input.options.wraps;
        let wrap_width = wraps.then_some(bounds.size.width);
        let mut lines = window.text_system().shape_text(text, font_size, &runs, wrap_width, None).ok()?.into_vec();
        if !wraps {
            lines.truncate(1);
        }
        let rows = Rows::new(lines.iter().map(|line| (line.len(), row_starts(line))));
        let line_height = window.line_height();
        Some(Layout { lines, rows, line_height, bounds, scroll_top: input.view.scroll_top, scroll_left: px(0.) })
    }

    fn settled(&self, mut layout: Layout, window: &Window, cx: &mut App) -> Layout {
        let input = self.input.read(cx);
        let focused = input.focus_handle.is_focused(window);
        let reveal = input.view.reveal || (focused && !input.view.was_focused);
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
        if let Some(scroller) = input.options.scroller.as_ref().filter(|_| reveal && focused) {
            let below = if input.options.rows.is_some() { PROMPT_MARGIN } else { SCROLLER_MARGIN };
            show_in_scroller(scroller, (layout.text_top() + caret, layout.line_height, below), window);
        }
        let (scroll_top, scroll_left) = (layout.scroll_top, layout.scroll_left);
        self.input.update(cx, |input, _| {
            input.view.scroll_top = scroll_top;
            input.view.scroll_left = scroll_left;
            input.view.reveal = false;
            input.view.was_focused = focused;
        });
        layout
    }

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

fn show_in_scroller(scroller: &ScrollHandle, (caret, line_height, below): (Pixels, Pixels, Pixels), window: &Window) {
    let view = scroller.bounds();
    let offset = scroller.offset();
    let scroll = -offset.y;
    let top = caret - view.top() + scroll;
    let margins = (SCROLLER_MARGIN, below);
    let next = scroll_to_show_with_margins(scroll, (top, line_height), margins, view.size.height).max(px(0.));
    if next != scroll {
        scroller.set_offset(point(offset.x, -next));
        window.request_animation_frame();
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

fn row_starts(line: &WrappedLine) -> Vec<usize> {
    let runs = &line.unwrapped_layout.runs;
    let wraps = line.wrap_boundaries().iter().map(|wrap| runs[wrap.run_ix].glyphs[wrap.glyph_ix].index);
    std::iter::once(0).chain(wraps).collect()
}
