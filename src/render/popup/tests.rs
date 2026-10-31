use gpui::{Bounds, point, px, size};

use super::*;
use crate::model::{Applied, Frame};

const CELL: (f32, f32) = (8., 16.);

fn popup_grid() -> Grid {
    let frame: Frame = serde_json::from_str(crate::test_util::fixture!("surface/popup.json")).expect("a frame");
    let mut grid = Grid::default();
    assert!(matches!(grid.apply(frame), Applied::Ok));
    grid
}

#[test]
fn the_popup_wants_its_terminal_and_the_padding_around_it() {
    assert_eq!(content_size(&popup_grid(), CELL), (60. * 8. + PADDING * 2., 10. * 16. + PADDING * 2.));
}

#[test]
fn a_popup_that_fits_keeps_its_own_size() {
    assert_eq!(fitted((504., 184.), (1200., 800.)), (504., 184.));
}

#[test]
fn a_popup_larger_than_the_terminal_is_cropped_to_the_room_left() {
    let room = (400., 300.);
    let width = 400. - MARGIN * 2. - INSET * 2.;
    let height = 300. - MARGIN * 2. - INSET * 2. - HEADER;
    assert_eq!(fitted((504., 400.), room), (width, height));
}

#[test]
fn no_room_gives_an_empty_popup_rather_than_a_negative_one() {
    assert_eq!(fitted((504., 184.), (10., 10.)), (0., 0.));
}

#[test]
fn a_press_in_the_popup_lands_on_its_own_cells() {
    let at = Bounds::new(point(px(100.), px(50.)), size(px(504.), px(184.)));
    let layout = Layout::new(&popup_grid(), CELL, at);
    let press = point(px(100. + PADDING + 8. * 3. + 1.), px(50. + PADDING + 16. * 2. + 1.));
    assert_eq!(layout.cell_in(40, press), Some((3, 2)));
    assert_eq!(layout.cards.len(), 1);
}
