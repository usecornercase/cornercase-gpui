use std::cell::RefCell;
use std::rc::Rc;

use gpui::{AnyElement, Bounds, Div, ParentElement as _, Pixels, Styled as _, canvas, div, fill, point, px, size};

use crate::theme::colours;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Separator {
    Dot,
    Line,
}

pub(crate) type Piece = (Option<Separator>, AnyElement);

const DOT: f32 = 3.;
const LINE_WIDTH: f32 = 1.;
const LINE_HEIGHT: f32 = 18.;
const LINE_ROOM: f32 = 5.;

impl Separator {
    fn width(self) -> f32 {
        match self {
            Self::Dot => DOT,
            Self::Line => LINE_WIDTH,
        }
    }

    fn room(self) -> f32 {
        match self {
            Self::Dot => DOT,
            Self::Line => LINE_ROOM,
        }
    }

    fn height(self) -> f32 {
        match self {
            Self::Dot => DOT,
            Self::Line => LINE_HEIGHT,
        }
    }
}

fn rooms(separators: &[Option<Separator>], gap: f32) -> Vec<f32> {
    (0..separators.len())
        .map(|i| separators.get(i + 1).copied().flatten().map_or(0., |next| next.room() + gap))
        .collect()
}

fn on_one_line(before: &Bounds<Pixels>, after: &Bounds<Pixels>) -> bool {
    after.left() >= before.right() && after.top() < before.bottom() && before.top() < after.bottom()
}

fn marks(separators: &[Option<Separator>], bounds: &[Bounds<Pixels>]) -> Vec<(Separator, Bounds<Pixels>)> {
    let mut marks = Vec::new();
    for (i, pair) in bounds.windows(2).enumerate() {
        let (before, after) = (&pair[0], &pair[1]);
        let Some(Some(separator)) = separators.get(i + 1) else { continue };
        if !on_one_line(before, after) {
            continue;
        }
        let x = f32::from(before.right() + after.left()) / 2. - separator.width() / 2.;
        let y = f32::from(before.center().y + after.center().y) / 2. - separator.height() / 2.;
        let origin = point(px(x.round()), px(y.round()));
        marks.push((*separator, Bounds::new(origin, size(px(separator.width()), px(separator.height())))));
    }
    marks
}

pub(crate) fn separated(gap: f32, pieces: Vec<Piece>) -> Div {
    let (separators, elements): (Vec<_>, Vec<_>) = pieces.into_iter().unzip();
    let laid: Rc<RefCell<Vec<Bounds<Pixels>>>> = Rc::default();
    let seen = laid.clone();
    let painted = separators.clone();
    let marker = canvas(
        |_, _, _| {},
        move |_, (), window, _| {
            let bounds = laid.borrow();
            for (separator, mark) in marks(&painted, bounds.get(1..).unwrap_or_default()) {
                match separator {
                    Separator::Dot => window.paint_quad(fill(mark, colours::faint()).corner_radii(px(DOT / 2.))),
                    Separator::Line => window.paint_quad(fill(mark, colours::border())),
                }
            }
        },
    )
    .absolute()
    .size_full();
    let room = rooms(&separators, gap);
    div()
        .relative()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(gap))
        .child(marker)
        .children(
            elements.into_iter().zip(room).map(|(element, room)| div().flex().min_w_0().mr(px(room)).child(element)),
        )
        .on_children_prepainted(move |bounds, _, _| *seen.borrow_mut() = bounds)
}

#[cfg(test)]
mod tests;
