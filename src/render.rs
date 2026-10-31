#[cfg(test)]
mod fixtures;
mod ghost;
pub(crate) mod layout;
pub(crate) mod moving;
pub(crate) mod popup;

use gpui::{
    App, BorderStyle, Bounds, ContentMask, Font, FontStyle, FontWeight, Hsla, Pixels, Point, Rgba, SharedString,
    TextRun, Window, fill, font, outline, point, px, quad, size, transparent_black,
};

use crate::model::link::Hovered;
use crate::model::{Cursor, CursorShape, Grid, PaneGrid, Run, UnderlineKind};
use crate::theme::{INACTIVE, Painter, colours, faded, hsla, radius};
pub(crate) use ghost::paint_ghost;
use layout::{Card, Layout, PADDING};
use moving::{Look, Stage, shrunk};

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

    pub(crate) fn cell(&self) -> (f32, f32) {
        (f32::from(self.cell_width), f32::from(self.line_height))
    }

    pub(crate) fn scaled(&self, by: f32) -> Self {
        Self {
            family: self.family.clone(),
            font_size: self.font_size * by,
            cell_width: self.cell_width * by,
            line_height: self.line_height * by,
        }
    }
}

fn offset(unit: Pixels, cells: usize) -> Pixels {
    unit * f32::from(u16::try_from(cells).unwrap_or(u16::MAX))
}

pub(crate) struct Surface<'a> {
    pub layout: &'a Layout,
    pub metrics: &'a Metrics,
    pub focused: bool,
    pub stage: &'a Stage,
}

pub(crate) fn card(bounds: Bounds<Pixels>, colour: impl Into<Hsla>, edge: Rgba, window: &mut Window) {
    let corner = px(radius::CARD);
    window.paint_quad(fill(bounds, colour.into()).corner_radii(corner));
    window.paint_quad(quad(bounds, corner, transparent_black(), px(1.), edge, BorderStyle::Solid));
}

pub(crate) fn paint_link(layout: &Layout, hovered: &Hovered, window: &mut Window) {
    let (row, start, end) = (hovered.link.row, hovered.link.start, hovered.link.end);
    let Some(span) = layout.span(hovered.pane, row, (start, end)) else { return };
    window.paint_quad(fill(span, faded(colours::accent().into(), 0.14)).corner_radii(px(3.)));
    let base = point(span.origin.x, span.origin.y + span.size.height - px(1.5));
    window.paint_quad(fill(Bounds::new(base, size(span.size.width, px(1.5))), colours::accent()));
}

pub(crate) fn paint(grid: &Grid, surface: &Surface<'_>, window: &mut Window, cx: &mut App) {
    for card in &surface.layout.cards {
        if let Some(pane) = grid.pane(card.pane) {
            paint_pane(pane, card, surface, window, cx);
        }
    }
    ghost::paint_marks(surface.stage, surface.layout, window);
}

fn pane_card(bounds: Bounds<Pixels>, (fill_colour, edge): (Hsla, Rgba), look: Look, window: &mut Window) {
    let corner = px(radius::CARD);
    window.paint_quad(fill(bounds, faded(fill_colour, 1. - 0.6 * look.hollow)).corner_radii(corner));
    let solid = faded(edge.into(), look.shown * (1. - look.hollow));
    window.paint_quad(quad(bounds, corner, transparent_black(), px(1.), solid, BorderStyle::Solid));
    if look.hollow > 0. {
        let dashed = faded(colours::accent().into(), 0.8 * look.hollow);
        window.paint_quad(quad(bounds, corner, transparent_black(), px(1.5), dashed, BorderStyle::Dashed));
    }
}

fn paint_pane(pane: &PaneGrid, card: &Card, surface: &Surface<'_>, window: &mut Window, cx: &mut App) {
    let (at, look) = surface.stage.shown(card);
    let several = surface.layout.cards.len() > 1;
    let painter = Painter { colours: &pane.colours };
    let edge = if several && pane.active { colours::edge() } else { colours::border() };
    let fade = if several && !pane.active { INACTIVE } else { 1. } * look.shown;
    let fill_colour = painter.pane_fill().unwrap_or_else(|| colours::pane().into());
    pane_card(at.bounds, (faded(fill_colour, fade), edge), look, window);
    let (cw, lh) = (surface.metrics.cell_width, surface.metrics.line_height);
    let mut cells = Bounds::new(at.cells, size(cw * f32::from(at.rect.cols), lh * f32::from(at.rect.rows)));
    if at.bounds != card.bounds {
        cells = cells.intersect(&shrunk(at.bounds, PADDING));
    }
    let canvas = PaneCanvas {
        metrics: surface.metrics,
        default_bg: hsla(painter.default_bg()),
        painter,
        top_left: at.cells,
        cursor: pane.cursor.filter(|_| pane.active || !several),
        focused: surface.focused,
        fade: fade * (1. - 0.8 * look.hollow),
    };
    window.with_content_mask(Some(ContentMask { bounds: cells }), |window| canvas.paint_rows(pane, window, cx));
}

struct PaneCanvas<'a> {
    metrics: &'a Metrics,
    painter: Painter<'a>,
    top_left: Point<Pixels>,
    default_bg: Hsla,
    cursor: Option<Cursor>,
    focused: bool,
    fade: f32,
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

impl<'a> PaneCanvas<'a> {
    fn plain(pane: &'a PaneGrid, metrics: &'a Metrics, top_left: Point<Pixels>, fade: f32) -> Self {
        let painter = Painter { colours: &pane.colours };
        let default_bg = hsla(painter.default_bg());
        Self { metrics, painter, top_left, default_bg, cursor: None, focused: false, fade }
    }

    fn paint_rows(&self, pane: &PaneGrid, window: &mut Window, cx: &mut App) {
        let rows = pane.lines.len().min(usize::from(pane.size.rows));
        for (row, line) in pane.lines.iter().take(rows).enumerate() {
            self.paint_line(row, line, window, cx);
        }
    }

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
        let fg = faded(fg, self.fade);
        if let Some(bg) = self.painter.cell_fill(bg, custom_bg) {
            window.paint_quad(fill(Bounds::new(at, size(width, lh)), faded(bg, self.fade)));
        }
        if s.underline {
            let colour = self.painter.underline_colour(s).map_or(fg, |c| faded(c, self.fade));
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
        let colour = faded(self.painter.cursor(), self.fade);
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
        let colour = faded(if cursor_cell == Some(col) { self.default_bg } else { fg }, self.fade);
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
