use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    Bounds, Div, InteractiveElement as _, ParentElement as _, Pixels, SharedString, StatefulInteractiveElement as _,
    Styled as _, TransformationMatrix, Window, canvas, div, fill, point, px, size,
};

use super::{Icon, glass};
use crate::theme::colours;

const ROUNDING: f32 = 1.;
const FADE: f32 = 18.;
const DISC: f32 = 16.;
const GLYPH: f32 = 10.;
const FROM_EDGE: f32 = 4.;
const BELOW_EDGE: f32 = 3.;

fn hidden_below(visible: Pixels, content: Pixels) -> bool {
    content - visible > px(ROUNDING)
}

#[derive(Clone, Copy)]
pub(crate) enum Seat {
    OnBorder,
    Inside,
}

impl Seat {
    fn centre(self, bottom: Pixels) -> Pixels {
        match self {
            Self::OnBorder => bottom + px(BELOW_EDGE),
            Self::Inside => bottom - px(FROM_EDGE + DISC / 2.),
        }
    }
}

pub(crate) fn cued(id: SharedString, content: Div, seat: Seat) -> Div {
    let seen: Rc<RefCell<Vec<Bounds<Pixels>>>> = Rc::default();
    let laid = seen.clone();
    let scroller = div()
        .id(id)
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .child(div().on_children_prepainted(move |bounds, _, _| *seen.borrow_mut() = bounds).child(content));
    let cue = canvas(
        |_, _, _| {},
        move |bounds, (), window, cx| {
            if laid.borrow().first().is_some_and(|c| hidden_below(bounds.bottom(), c.bottom())) {
                paint(bounds, seat, window, cx);
            }
        },
    )
    .absolute()
    .size_full();
    div().relative().flex().flex_col().min_h_0().child(scroller).child(cue)
}

fn paint(bounds: Bounds<Pixels>, seat: Seat, window: &mut Window, cx: &mut gpui::App) {
    let band = Bounds::from_corners(point(bounds.left(), bounds.bottom() - px(FADE)), bounds.bottom_right());
    window.paint_quad(fill(band, glass::fade_to_card()));
    let centre = point(bounds.center().x, seat.centre(bounds.bottom()));
    let disc = Bounds::centered_at(centre, size(px(DISC), px(DISC)));
    window.paint_quad(
        fill(disc, colours::card_bottom())
            .corner_radii(px(DISC / 2.))
            .border_widths(px(1.))
            .border_color(colours::border()),
    );
    let glyph = Bounds::centered_at(centre, size(px(GLYPH), px(GLYPH)));
    let path = SharedString::from(Icon::ChevronDown.path());
    let _ = window.paint_svg(glyph, path, TransformationMatrix::unit(), colours::muted().into(), cx);
}

#[cfg(test)]
mod tests;
