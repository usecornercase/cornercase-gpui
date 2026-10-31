use gpui::{App, Bounds, ContentMask, Pixels, Window};

use super::layout::{Layout, PADDING};
use super::moving::Stage;
use super::{Metrics, Surface, paint};
use crate::model::Grid;

pub(crate) const MARGIN: f32 = 24.;
pub(crate) const HEADER: f32 = 40.;
pub(crate) const INSET: f32 = 8.;

pub(crate) fn content_size(grid: &Grid, cell: (f32, f32)) -> (f32, f32) {
    (f32::from(grid.cols) * cell.0 + PADDING * 2., f32::from(grid.rows) * cell.1 + PADDING * 2.)
}

pub(crate) fn fitted(want: (f32, f32), room: (f32, f32)) -> (f32, f32) {
    let width = room.0 - MARGIN * 2. - INSET * 2.;
    let height = room.1 - MARGIN * 2. - INSET * 2. - HEADER;
    (want.0.min(width).max(0.), want.1.min(height).max(0.))
}

pub(crate) fn paint_popup(
    grid: &Grid,
    (metrics, focused): (&Metrics, bool),
    bounds: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) -> Layout {
    let layout = Layout::new(grid, metrics.cell(), bounds);
    let surface = Surface { layout: &layout, metrics, focused, stage: &Stage::default() };
    window.with_content_mask(Some(ContentMask { bounds }), |window| paint(grid, &surface, window, cx));
    layout
}

#[cfg(test)]
mod tests;
