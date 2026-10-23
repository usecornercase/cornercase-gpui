use gpui::{
    App, BorderStyle, Bounds, ContentMask, Font, FontStyle, FontWeight, Hsla, Pixels, Point, SharedString, TextRun,
    Window, fill, font, outline, point, px, size,
};

use crate::model::{Area, Cursor, CursorShape, Grid, PaneGrid, Run, UnderlineKind};
use crate::theme::{Painter, colours, hsla};

#[derive(Clone)]
pub(crate) struct Metrics {
    pub family: SharedString,
    pub font_size: Pixels,
    pub cell_width: Pixels,
    pub line_height: Pixels,
}

impl Metrics {
    pub(crate) fn measure(family: &str, font_size: f32, window: &Window) -> Self {
        let font_size = px(font_size);
        let base = font(SharedString::from(family.to_string()));
        let text = window.text_system();
        let id = text.resolve_font(&base);
        let cell_width = text.advance(id, font_size, 'M').map_or(font_size * 0.6, |s| s.width);
        Self {
            family: SharedString::from(family.to_string()),
            font_size,
            cell_width,
            line_height: (font_size * 1.3).round(),
        }
    }

    fn font(&self, bold: bool, italic: bool) -> Font {
        let mut f = font(self.family.clone());
        if bold {
            f.weight = FontWeight::BOLD;
        }
        if italic {
            f.style = FontStyle::Italic;
        }
        f
    }

    pub(crate) fn cell_at(&self, origin: Point<Pixels>, position: Point<Pixels>) -> (i32, i32) {
        let dx = f32::from(position.x - origin.x) / f32::from(self.cell_width);
        let dy = f32::from(position.y - origin.y) / f32::from(self.line_height);
        (floor_to_cell(dx), floor_to_cell(dy))
    }

    fn area(&self, origin: Point<Pixels>, area: Area) -> Bounds<Pixels> {
        let (cw, lh) = (self.cell_width, self.line_height);
        Bounds::new(
            origin + point(cw * f32::from(area.x), lh * f32::from(area.y)),
            size(cw * f32::from(area.cols), lh * f32::from(area.rows)),
        )
    }
}

#[expect(clippy::cast_possible_truncation, reason = "a pointer's offset in cells is far inside i32")]
fn floor_to_cell(cells: f32) -> i32 {
    cells.floor() as i32
}

fn offset(unit: Pixels, cells: usize) -> Pixels {
    unit * f32::from(u16::try_from(cells).unwrap_or(u16::MAX))
}

pub(crate) struct Surface<'a> {
    pub origin: Point<Pixels>,
    pub metrics: &'a Metrics,
    pub focused: bool,
}

pub(crate) fn paint(grid: &Grid, surface: &Surface<'_>, window: &mut Window, cx: &mut App) {
    for divider in &grid.dividers {
        let b = surface.metrics.area(surface.origin, divider.line);
        window.paint_quad(fill(b, colours::terminal()));
        let center = if divider.dir == "down" {
            Bounds::new(point(b.origin.x, b.origin.y + b.size.height / 2.), size(b.size.width, px(1.)))
        } else {
            Bounds::new(point(b.origin.x + b.size.width / 2., b.origin.y), size(px(1.), b.size.height))
        };
        window.paint_quad(fill(center, colours::border()));
    }
    let several = grid.panes.len() > 1;
    for pane in &grid.panes {
        paint_pane(pane, surface, several, window, cx);
    }
}

fn paint_pane(pane: &PaneGrid, surface: &Surface<'_>, several: bool, window: &mut Window, cx: &mut App) {
    let bounds = surface.metrics.area(surface.origin, pane.rect);
    let painter = Painter { colours: &pane.colours };
    let canvas = PaneCanvas {
        metrics: surface.metrics,
        default_bg: hsla(painter.default_bg()),
        painter,
        top_left: bounds.origin,
        cursor: pane.cursor.filter(|_| pane.active || !several),
        focused: surface.focused,
    };
    window.paint_quad(fill(bounds, canvas.default_bg));
    window.with_content_mask(Some(ContentMask { bounds }), |window| {
        let rows = pane.lines.len().min(usize::from(pane.size.rows));
        for (row, line) in pane.lines.iter().take(rows).enumerate() {
            canvas.paint_line(row, line, window, cx);
        }
    });
    if several && !pane.active {
        window.paint_quad(fill(bounds, Hsla { h: 0., s: 0., l: 0., a: 0.18 }));
    }
}

struct PaneCanvas<'a> {
    metrics: &'a Metrics,
    painter: Painter<'a>,
    top_left: Point<Pixels>,
    default_bg: Hsla,
    cursor: Option<Cursor>,
    focused: bool,
}

struct RowAt {
    y: Pixels,
    cursor_cell: Option<usize>,
}

struct Glyph<'a> {
    col: usize,
    text: &'a str,
    font: Font,
    colour: Hsla,
}

fn hline(window: &mut Window, at: Point<Pixels>, width: Pixels, colour: Hsla) {
    window.paint_quad(fill(Bounds::new(at, size(width, px(1.))), colour));
}

impl PaneCanvas<'_> {
    fn paint_line(&self, row: usize, line: &[Run], window: &mut Window, cx: &mut App) {
        let y = self.top_left.y + offset(self.metrics.line_height, row);
        let mut col = 0usize;
        for run in line {
            let at = point(self.top_left.x + offset(self.metrics.cell_width, col), y);
            self.paint_decorations(run, at, window);
            col += run.cells.len();
        }
        let cursor_cell = self.paint_cursor(row, y, window);
        self.paint_text(line, &RowAt { y, cursor_cell }, window, cx);
    }

    fn paint_decorations(&self, run: &Run, at: Point<Pixels>, window: &mut Window) {
        let lh = self.metrics.line_height;
        let width = offset(self.metrics.cell_width, run.cells.len());
        let s = &run.style;
        let (fg, bg, custom_bg) = self.painter.resolve(s);
        if custom_bg {
            window.paint_quad(fill(Bounds::new(at, size(width, lh)), bg));
        }
        if s.underline {
            let colour = self.painter.underline_colour(s).unwrap_or(fg);
            let base = point(at.x, at.y + lh - px(2.));
            hline(window, base, width, colour);
            if s.underline_kind == Some(UnderlineKind::Double) {
                hline(window, point(base.x, base.y - px(2.)), width, colour);
            }
        }
        if s.strikethrough {
            hline(window, point(at.x, at.y + lh / 2.), width, fg);
        }
        if s.overline {
            hline(window, at, width, fg);
        }
    }

    fn paint_cursor(&self, row: usize, y: Pixels, window: &mut Window) -> Option<usize> {
        let c = self.cursor.filter(|c| usize::from(c.y) == row)?;
        let (cw, lh) = (self.metrics.cell_width, self.metrics.line_height);
        let cb = Bounds::new(point(self.top_left.x + cw * f32::from(c.x), y), size(cw, lh));
        let colour = self.painter.cursor();
        match (c.shape, self.focused) {
            (CursorShape::Block, true) => {
                window.paint_quad(fill(cb, colour));
                return Some(usize::from(c.x));
            }
            (CursorShape::Bar, true) => window.paint_quad(fill(Bounds::new(cb.origin, size(px(2.), lh)), colour)),
            (CursorShape::Underline, true) => window
                .paint_quad(fill(Bounds::new(point(cb.origin.x, cb.origin.y + lh - px(2.)), size(cw, px(2.))), colour)),
            _ => window.paint_quad(outline(cb, colour, BorderStyle::Solid)),
        }
        None
    }

    fn paint_text(&self, line: &[Run], row: &RowAt, window: &mut Window, cx: &mut App) {
        let cells: Vec<(&str, &Run)> =
            line.iter().flat_map(|run| run.cells.iter().map(move |c| (c.as_str(), run))).collect();
        let origin = point(self.top_left.x, row.y);
        let mut segment = Segment::default();
        for (i, (text, run)) in cells.iter().enumerate() {
            if text.is_empty() || *text == " " {
                segment.flush(origin, self.metrics, window, cx);
                continue;
            }
            let glyph = self.glyph(i, text, run, row.cursor_cell);
            let wide = cells.get(i + 1).is_some_and(|(t, _)| t.is_empty());
            if wide || text.chars().count() > 1 {
                segment.flush(origin, self.metrics, window, cx);
                self.paint_alone(&glyph, origin, window, cx);
                continue;
            }
            segment.push(glyph);
        }
        segment.flush(origin, self.metrics, window, cx);
    }

    fn glyph<'t>(&self, col: usize, text: &'t str, run: &Run, cursor_cell: Option<usize>) -> Glyph<'t> {
        let (fg, _, _) = self.painter.resolve(&run.style);
        let colour = if cursor_cell == Some(col) { self.default_bg } else { fg };
        Glyph { col, text, font: self.metrics.font(run.style.bold, run.style.italic), colour }
    }

    fn paint_alone(&self, glyph: &Glyph<'_>, origin: Point<Pixels>, window: &mut Window, cx: &mut App) {
        let runs = [text_run(glyph.text.len(), glyph.font.clone(), glyph.colour)];
        let text = SharedString::from(glyph.text.to_string());
        let shaped = window.text_system().shape_line(text, self.metrics.font_size, &runs, None);
        let at = point(origin.x + offset(self.metrics.cell_width, glyph.col), origin.y);
        let _ = shaped.paint(at, self.metrics.line_height, window, cx);
    }
}

fn text_run(len: usize, font: Font, color: Hsla) -> TextRun {
    TextRun { len, font, color, background_color: None, underline: None, strikethrough: None }
}

#[derive(Default)]
struct Segment {
    start: usize,
    text: String,
    runs: Vec<TextRun>,
}

impl Segment {
    fn push(&mut self, glyph: Glyph<'_>) {
        if self.text.is_empty() {
            self.start = glyph.col;
        }
        self.text.push_str(glyph.text);
        let len = glyph.text.len();
        match self.runs.last_mut() {
            Some(last) if last.font == glyph.font && last.color == glyph.colour => last.len += len,
            _ => self.runs.push(text_run(len, glyph.font, glyph.colour)),
        }
    }

    fn flush(&mut self, origin: Point<Pixels>, metrics: &Metrics, window: &mut Window, cx: &mut App) {
        if self.text.is_empty() {
            return;
        }
        let text = SharedString::from(std::mem::take(&mut self.text));
        let runs = std::mem::take(&mut self.runs);
        let shaped = window.text_system().shape_line(text, metrics.font_size, &runs, Some(metrics.cell_width));
        let at = point(origin.x + offset(metrics.cell_width, self.start), origin.y);
        let _ = shaped.paint(at, metrics.line_height, window, cx);
    }
}
