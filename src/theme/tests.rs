use std::collections::BTreeMap;

use super::*;

fn painter_with(bg: Option<[u8; 3]>) -> Colours {
    Colours { fg: None, bg, cursor: None, palette: BTreeMap::new() }
}

#[test]
fn the_opacity_defaults_when_unset_or_unreadable() {
    assert!((opacity_from(None) - OPACITY).abs() < f32::EPSILON);
    assert!((opacity_from(Some("glassy")) - OPACITY).abs() < f32::EPSILON);
    assert!((opacity_from(Some("NaN")) - OPACITY).abs() < f32::EPSILON);
}

#[test]
fn the_opacity_is_read_and_kept_readable() {
    assert!((opacity_from(Some(" 0.7 ")) - 0.7).abs() < f32::EPSILON);
    assert!((opacity_from(Some("4")) - 1.).abs() < f32::EPSILON);
    assert!((opacity_from(Some("0")) - LEAST_OPACITY).abs() < f32::EPSILON);
}

#[test]
fn a_pane_on_the_window_background_is_left_translucent() {
    let colours = painter_with(None);
    assert_eq!(Painter { colours: &colours }.pane_fill(), None);
}

#[test]
fn a_pane_whose_program_chose_a_background_is_filled_with_it() {
    let colours = painter_with(Some([255, 255, 255]));
    assert_eq!(Painter { colours: &colours }.pane_fill(), Some(hsla([255, 255, 255])));
}

#[test]
fn a_cell_painted_in_the_default_background_stays_translucent() {
    let colours = painter_with(None);
    let painter = Painter { colours: &colours };
    assert_eq!(painter.cell_fill(hsla(BACKGROUND), true), None);
    assert_eq!(painter.cell_fill(hsla(RED), false), None);
    assert_eq!(painter.cell_fill(hsla(RED), true), Some(hsla(RED)));
}

fn over(top: Rgba, below: [f32; 3]) -> [f32; 3] {
    let mix = |c: f32, b: f32| c * top.a + b * (1. - top.a);
    [mix(top.r, below[0]), mix(top.g, below[1]), mix(top.b, below[2])]
}

fn close(a: [f32; 3], b: [f32; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-4)
}

#[test]
fn a_pane_over_the_ground_looks_as_it_would_over_the_desktop() {
    let wanted = Rgba { a: 0.82, ..rgb(0x1a_1b_26) };
    let ground = colours::glass();
    let card = over_ground(wanted, ground);
    for desktop in [[0., 0., 0.], [1., 1., 1.], [0.2, 0.6, 0.9]] {
        assert!(close(over(card, over(ground, desktop)), over(wanted, desktop)));
    }
}

#[test]
fn an_opaque_pane_keeps_its_colour_over_the_ground() {
    let wanted = rgb(0x1a_1b_26);
    assert_eq!(over_ground(wanted, colours::glass()), wanted);
}

#[test]
fn a_pane_more_see_through_than_the_ground_shows_only_the_ground() {
    let wanted = Rgba { a: 0.4, ..rgb(0x1a_1b_26) };
    assert!(over_ground(wanted, colours::glass()).a.abs() < f32::EPSILON);
}

#[test]
fn an_inactive_pane_fades_its_colours_without_darkening_them() {
    let colour = hsla(RED);
    let dim = faded(colour, INACTIVE);
    assert!((dim.a - INACTIVE).abs() < f32::EPSILON);
    assert_eq!(Hsla { a: 1., ..dim }, colour);
}
