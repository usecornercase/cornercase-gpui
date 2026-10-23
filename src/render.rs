//! Paints a surface frame: each pane's cells at its place, the dividers and the cursors.

use gpui::{
    App, BorderStyle, Bounds, ContentMask, Font, FontStyle, FontWeight, Hsla, Pixels, Point, SharedString, TextRun,
    Window, fill, font, outline, point, px, size,
};

use crate::model::{CursorShape, Grid, PaneGrid, Run, UnderlineKind};
use crate::theme::{Painter, colours};

/// The terminal font and the size of one cell.
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

    /// The cell under a point, relative to the surface's origin; may be outside the surface.
    pub(crate) fn cell_at(&self, origin: Point<Pixels>, position: Point<Pixels>) -> (i32, i32) {
        let dx = f32::from(position.x - origin.x) / f32::from(self.cell_width);
        let dy = f32::from(position.y - origin.y) / f32::from(self.line_height);
        (dx.floor() as i32, dy.floor() as i32)
    }
}

/// Paints the grid with its top-left cell at `origin`.
pub(crate) fn paint(
    grid: &Grid,
    origin: Point<Pixels>,
    metrics: &Metrics,
    focused: bool,
    window: &mut Window,
    cx: &mut App,
) {
    let cw = metrics.cell_width;
    let lh = metrics.line_height;
    for divider in &grid.dividers {
        let line = divider.line;
        let b = Bounds::new(
            origin + point(cw * f32::from(line.x), lh * f32::from(line.y)),
            size(cw * f32::from(line.cols), lh * f32::from(line.rows)),
        );
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
        paint_pane(pane, origin, metrics, focused, several, window, cx);
    }
}

fn paint_pane(
    pane: &PaneGrid,
    origin: Point<Pixels>,
    metrics: &Metrics,
    focused: bool,
    several: bool,
    window: &mut Window,
    cx: &mut App,
) {
    let cw = metrics.cell_width;
    let lh = metrics.line_height;
    let rect = pane.rect;
    let top_left = origin + point(cw * f32::from(rect.x), lh * f32::from(rect.y));
    let bounds = Bounds::new(top_left, size(cw * f32::from(rect.cols), lh * f32::from(rect.rows)));
    let painter = Painter { colours: &pane.colours };
    let default_bg = crate::theme::hsla(painter.default_bg());
    window.paint_quad(fill(bounds, default_bg));

    window.with_content_mask(Some(ContentMask { bounds }), |window| {
        let rows = pane.lines.len().min(pane.size.rows as usize);
        let cursor = pane.cursor.filter(|_| pane.active || !several);
        for (row, line) in pane.lines.iter().take(rows).enumerate() {
            let y = top_left.y + lh * row as f32;
            let cells: Vec<(&str, &Run)> =
                line.iter().flat_map(|run| run.cells.iter().map(move |c| (c.as_str(), run))).collect();

            // Backgrounds and decorations, run by run.
            let mut col = 0usize;
            for run in line {
                let n = run.cells.len();
                let x = top_left.x + cw * col as f32;
                let width = cw * n as f32;
                let (fg, bg, custom_bg) = painter.resolve(&run.style);
                if custom_bg {
                    window.paint_quad(fill(Bounds::new(point(x, y), size(width, lh)), bg));
                }
                let s = &run.style;
                if s.underline {
                    let colour = painter.underline_colour(s).unwrap_or(fg);
                    let base = y + lh - px(2.);
                    window.paint_quad(fill(Bounds::new(point(x, base), size(width, px(1.))), colour));
                    if s.underline_style == Some(UnderlineKind::Double) {
                        window.paint_quad(fill(Bounds::new(point(x, base - px(2.)), size(width, px(1.))), colour));
                    }
                }
                if s.strikethrough {
                    window.paint_quad(fill(Bounds::new(point(x, y + lh / 2.), size(width, px(1.))), fg));
                }
                if s.overline {
                    window.paint_quad(fill(Bounds::new(point(x, y), size(width, px(1.))), fg));
                }
                col += n;
            }

            // The cursor goes under the text, which is drawn over it in the background colour.
            let mut cursor_cell = None;
            if let Some(c) = cursor.filter(|c| c.y as usize == row) {
                let cb = Bounds::new(point(top_left.x + cw * f32::from(c.x), y), size(cw, lh));
                let colour = painter.cursor();
                match (c.shape, focused) {
                    (CursorShape::Block, true) => {
                        window.paint_quad(fill(cb, colour));
                        cursor_cell = Some(c.x as usize);
                    }
                    (CursorShape::Bar, true) => {
                        window.paint_quad(fill(Bounds::new(cb.origin, size(px(2.), lh)), colour))
                    }
                    (CursorShape::Underline, true) => window.paint_quad(fill(
                        Bounds::new(point(cb.origin.x, cb.origin.y + lh - px(2.)), size(cw, px(2.))),
                        colour,
                    )),
                    _ => window.paint_quad(outline(cb, colour, BorderStyle::Solid)),
                }
            }

            // Text: words of single-width cells shaped together with a forced advance, so they
            // stay on the grid whatever font a glyph falls back to; wide and composed cells alone.
            let mut seg = Segment::default();
            for (i, (text, run)) in cells.iter().enumerate() {
                if text.is_empty() || *text == " " {
                    seg.flush(top_left, y, metrics, window, cx);
                    continue;
                }
                let (mut fg, _, _) = painter.resolve(&run.style);
                if cursor_cell == Some(i) {
                    fg = default_bg;
                }
                let f = metrics.font(run.style.bold, run.style.italic);
                let wide = cells.get(i + 1).is_some_and(|(t, _)| t.is_empty());
                if wide || text.chars().count() > 1 {
                    seg.flush(top_left, y, metrics, window, cx);
                    let runs = [text_run(text.len(), f, fg)];
                    let shaped = window.text_system().shape_line(
                        SharedString::from(text.to_string()),
                        metrics.font_size,
                        &runs,
                        None,
                    );
                    let _ = shaped.paint(point(top_left.x + cw * i as f32, y), lh, window, cx);
                    continue;
                }
                seg.push(i, text, f, fg);
            }
            seg.flush(top_left, y, metrics, window, cx);
        }
    });

    if several && !pane.active {
        window.paint_quad(fill(bounds, Hsla { h: 0., s: 0., l: 0., a: 0.18 }));
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
    fn push(&mut self, col: usize, cell: &str, font: Font, color: Hsla) {
        if self.text.is_empty() {
            self.start = col;
        }
        self.text.push_str(cell);
        match self.runs.last_mut() {
            Some(last) if last.font == font && last.color == color => last.len += cell.len(),
            _ => self.runs.push(text_run(cell.len(), font, color)),
        }
    }

    fn flush(&mut self, top_left: Point<Pixels>, y: Pixels, metrics: &Metrics, window: &mut Window, cx: &mut App) {
        if self.text.is_empty() {
            return;
        }
        let text = SharedString::from(std::mem::take(&mut self.text));
        let runs = std::mem::take(&mut self.runs);
        let shaped = window.text_system().shape_line(text, metrics.font_size, &runs, Some(metrics.cell_width));
        let x = top_left.x + metrics.cell_width * self.start as f32;
        let _ = shaped.paint(point(x, y), metrics.line_height, window, cx);
    }
}
