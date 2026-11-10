use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    App, Bounds, ContentMask, Context, Div, IntoElement, ParentElement as _, Percentage, Pixels, Render, Rgba,
    Styled as _, TransformationMatrix, Window, canvas, px,
};

use super::{Icon, motion};
use crate::theme::colours;

const GLYPH: f32 = 12.;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Spot {
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) mask: Bounds<Pixels>,
    pub(crate) colour: Rgba,
}

impl Spot {
    pub(crate) fn shows(&self) -> bool {
        self.bounds.intersects(&self.mask)
    }
}

#[derive(Clone, Default)]
pub(crate) struct Spots(Rc<RefCell<Vec<Spot>>>);

impl Spots {
    pub(crate) fn clear(&self) {
        self.0.borrow_mut().clear();
    }

    pub(crate) fn push(&self, spot: Spot) {
        self.0.borrow_mut().push(spot);
    }

    pub(crate) fn shown(&self) -> Vec<Spot> {
        self.0.borrow().iter().copied().filter(Spot::shows).collect()
    }
}

pub(crate) fn orbit_matrix(bounds: Bounds<Pixels>, scale: f32, turned: f32) -> TransformationMatrix {
    let centre = bounds.center();
    TransformationMatrix::unit()
        .translate(centre.scale(scale))
        .rotate(Percentage(turned).into())
        .translate(centre.scale(-scale))
}

pub(crate) fn badge(status: Option<&str>, spots: &Spots, dim: f32) -> Div {
    if status != Some("working") {
        return super::still_badge(status);
    }
    let spots = spots.clone();
    let colour = colours::working();
    let colour = Rgba { a: colour.a * dim, ..colour };
    let probe = canvas(
        move |bounds, window, _| spots.push(Spot { bounds, mask: window.content_mask().bounds, colour }),
        |_, (), _, _| {},
    )
    .size(px(GLYPH))
    .flex_none();
    super::badge_slot().child(probe)
}

pub(crate) struct OrbitLayer {
    spots: Spots,
}

impl OrbitLayer {
    pub(crate) fn new(spots: Spots) -> Self {
        Self { spots }
    }
}

fn paint_spots(spots: &Spots, window: &mut Window, cx: &mut App) {
    let turned = motion::orbit_turned(motion::clock());
    let scale = window.scale_factor();
    let shown = spots.shown();
    for spot in &shown {
        let matrix = orbit_matrix(spot.bounds, scale, turned);
        window.with_content_mask(Some(ContentMask { bounds: spot.mask }), |window| {
            window.paint_svg(spot.bounds, Icon::Working.path().into(), matrix, spot.colour.into(), cx).ok();
        });
    }
    motion::orbits_shown(shown.len());
}

impl Render for OrbitLayer {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let spots = self.spots.clone();
        canvas(|_, _, _| {}, move |_, (), window, cx| paint_spots(&spots, window, cx)).size_full()
    }
}

#[cfg(test)]
mod tests;
