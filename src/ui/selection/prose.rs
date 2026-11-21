use std::sync::Arc;

use gpui::{
    AnyElement, CursorStyle, Div, IntoElement, ParentElement as _, Pixels, Point, SharedString, Styled as _,
    StyledText, TextLayout, WrappedLineLayout, canvas, div, point, px,
};

use super::pointer::Laid;
use super::{Lit, Styles, lit_styles};

type Probe = Box<dyn FnOnce(usize, TextLayout) -> AnyElement>;

pub(crate) struct Pick {
    lit: Lit,
    probe: Probe,
}

pub(crate) fn pick<S: Copy + Ord + 'static>(laid: &Laid<S>, spot: S, lit: Lit) -> Pick {
    let laid = laid.clone();
    let probe: Probe = Box::new(move |len, layout| {
        canvas(move |bounds, _, _| laid.add_prose((spot, len), bounds, layout), |_, (), _, _| {})
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .into_any_element()
    });
    Pick { lit, probe }
}

impl Pick {
    pub(crate) fn draw(
        self,
        text: impl Into<SharedString>,
        styles: Styles,
        wrap: impl FnOnce(StyledText) -> AnyElement,
    ) -> Div {
        let text: SharedString = text.into();
        let len = text.len();
        let drawn = StyledText::new(text).with_highlights(lit_styles(styles, &self.lit, len));
        let layout = drawn.layout().clone();
        div().relative().cursor(CursorStyle::IBeam).child(wrap(drawn)).child((self.probe)(len, layout))
    }
}

pub(super) fn line_at(lines: &[(usize, usize)], height: Pixels, y: Pixels) -> Option<(usize, usize, Pixels)> {
    let (mut top, mut start) = (px(0.), 0);
    for (ix, &(len, rows)) in lines.iter().enumerate() {
        let bottom = top + height * rows;
        if y < bottom || ix + 1 == lines.len() {
            return Some((ix, start, top));
        }
        (top, start) = (bottom, start + len + 1);
    }
    None
}

fn laid_lines(layout: &TextLayout) -> Vec<Arc<WrappedLineLayout>> {
    let (mut lines, mut start, len) = (Vec::new(), 0, layout.len());
    while start <= len {
        let Some(line) = layout.line_layout_for_index(start) else { break };
        start += line.len() + 1;
        lines.push(line);
    }
    lines
}

pub(super) fn prose_index(layout: &TextLayout, at: Point<Pixels>) -> usize {
    let lines = laid_lines(layout);
    let shape: Vec<(usize, usize)> = lines.iter().map(|line| (line.len(), line.wrap_boundaries().len() + 1)).collect();
    let (height, origin) = (layout.line_height(), layout.bounds().origin);
    let Some((ix, start, top)) = line_at(&shape, height, at.y - origin.y) else { return 0 };
    let rows = shape[ix].1;
    let y = (at.y - origin.y - top).clamp(px(0.), height * rows - px(1.));
    start + lines[ix].closest_index_for_position(point(at.x - origin.x, y), height).unwrap_or_else(|index| index)
}

#[cfg(test)]
mod tests;
