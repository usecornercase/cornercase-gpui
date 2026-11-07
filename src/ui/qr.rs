use std::rc::Rc;

use gpui::{Bounds, IntoElement, Pixels, Styled as _, Window, canvas, fill, point, px, size};
use qrcode::{Color, EcLevel, QrCode};

use crate::theme::{colours, radius};

const QUIET: u16 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Matrix {
    width: u16,
    dark: Vec<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Run {
    x: u16,
    y: u16,
    length: u16,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Fit {
    module: f32,
    side: f32,
}

impl Matrix {
    pub(crate) fn of(text: &str) -> Option<Self> {
        let code = QrCode::with_error_correction_level(text.as_bytes(), EcLevel::L).ok()?;
        let width = u16::try_from(code.width()).ok()?;
        Some(Self { width, dark: code.to_colors().into_iter().map(|module| module == Color::Dark).collect() })
    }

    fn side(&self) -> u16 {
        self.width + 2 * QUIET
    }

    fn runs(&self) -> Vec<Run> {
        (0u16..).zip(self.dark.chunks(usize::from(self.width))).flat_map(|(y, row)| runs_of(y, row)).collect()
    }
}

fn runs_of(y: u16, row: &[bool]) -> Vec<Run> {
    let mut x = 0;
    let mut runs = Vec::new();
    for same in row.chunk_by(|a, b| a == b) {
        let length = u16::try_from(same.len()).unwrap_or(u16::MAX);
        if same[0] {
            runs.push(Run { x, y, length });
        }
        x += length;
    }
    runs
}

fn fit(side: u16, room: f32, scale: f32) -> Fit {
    let side = f32::from(side);
    let module = (room * scale / side).floor().max(1.) / scale;
    Fit { module, side: module * side }
}

fn snapped(at: Pixels, scale: f32) -> f32 {
    (f32::from(at) * scale).round() / scale
}

fn square(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
}

fn paint(matrix: &Matrix, fit: Fit, bounds: Bounds<Pixels>, window: &mut Window) {
    let scale = window.scale_factor();
    let (left, top) = (snapped(bounds.origin.x, scale), snapped(bounds.origin.y, scale));
    let ground = square(left, top, fit.side, fit.side);
    window.paint_quad(fill(ground, colours::code_ground()).corner_radii(px(radius::CONTROL)));
    let at = |module: u16| f32::from(QUIET + module) * fit.module;
    for run in matrix.runs() {
        let bar = square(left + at(run.x), top + at(run.y), f32::from(run.length) * fit.module, fit.module);
        window.paint_quad(fill(bar, colours::code_ink()));
    }
}

pub(crate) fn code(matrix: Rc<Matrix>, room: f32, window: &Window) -> impl IntoElement + use<> {
    let fit = fit(matrix.side(), room, window.scale_factor());
    canvas(|_, _, _| {}, move |bounds, (), window, _| paint(&matrix, fit, bounds, window))
        .size(px(fit.side))
        .flex_none()
}

#[cfg(test)]
mod tests;
