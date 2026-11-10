use gpui::{Bounds, Pixels, Rgba, TransformationMatrix, point, px, size};

use super::{Spot, Spots, orbit_matrix};
use crate::ui::motion::{Beat, orbit, orbits_drawn, orbits_shown};

fn bounds(x: f32, y: f32, edge: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(edge), px(edge)))
}

fn spot(at: Bounds<Pixels>, mask: Bounds<Pixels>) -> Spot {
    Spot { bounds: at, mask, colour: Rgba { r: 1., g: 0., b: 0., a: 1. } }
}

fn through(matrix: &TransformationMatrix, at: (f32, f32)) -> (f32, f32) {
    let [top, bottom] = matrix.rotation_scale;
    let [shift_x, shift_y] = matrix.translation;
    (top[0] * at.0 + top[1] * at.1 + shift_x, bottom[0] * at.0 + bottom[1] * at.1 + shift_y)
}

fn near(got: (f32, f32), want: (f32, f32)) {
    assert!((got.0 - want.0).abs() < 1e-3 && (got.1 - want.1).abs() < 1e-3, "{got:?} is not {want:?}");
}

#[test]
fn a_spot_out_of_its_list_does_not_show() {
    assert!(!spot(bounds(10., 300., 12.), bounds(0., 0., 200.)).shows());
}

#[test]
fn a_spot_half_in_view_shows() {
    assert!(spot(bounds(10., 195., 12.), bounds(0., 0., 200.)).shows());
}

#[test]
fn the_layer_counts_only_spots_that_show() {
    let mask = bounds(0., 0., 200.);
    let spots = Spots::default();
    spots.push(spot(bounds(10., 10., 12.), mask));
    spots.push(spot(bounds(10., 400., 12.), mask));
    assert_eq!(spots.shown().len(), 1);
    spots.clear();
    assert_eq!(spots.shown().len(), 0);
}

#[test]
fn the_orbit_turns_clockwise_about_the_badge_centre() {
    let matrix = orbit_matrix(bounds(10., 20., 12.), 2., 0.25);
    near(through(&matrix, (32., 52.)), (32., 52.));
    near(through(&matrix, (32., 40.)), (44., 52.));
}

#[test]
fn no_turn_leaves_the_badge_in_place() {
    let matrix = orbit_matrix(bounds(10., 20., 12.), 2., 0.);
    near(through(&matrix, (30., 40.)), (30., 40.));
    assert_eq!(matrix.rotation_scale, [[1., 0.], [0., 1.]]);
}

#[test]
fn spots_shown_count_with_the_badges_drawn() {
    Beat::default().drawing(std::time::Instant::now());
    drop(orbit(gpui::svg()));
    orbits_shown(2);
    assert_eq!(orbits_drawn(), 3);
}
