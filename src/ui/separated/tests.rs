use gpui::{Bounds, Pixels, point, px, size};

use super::{Separator, marks, rooms};

const DOT: Option<Separator> = Some(Separator::Dot);
const LINE: Option<Separator> = Some(Separator::Line);

fn at(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
}

fn origins(found: &[(Separator, Bounds<Pixels>)]) -> Vec<(f32, f32)> {
    found.iter().map(|(_, b)| (f32::from(b.origin.x), f32::from(b.origin.y))).collect()
}

#[test]
fn a_separator_shows_between_two_parts_on_one_line() {
    let bounds = [at(0., 0., 50., 16.), at(65., 0., 40., 16.), at(120., 0., 30., 16.)];
    let found = marks(&[None, DOT, DOT], &bounds);
    assert_eq!(found.len(), 2);
    assert_eq!(origins(&found), vec![(56., 7.), (111., 7.)]);
}

#[test]
fn a_wrap_drops_the_separator_at_both_ends_of_the_lines() {
    let bounds = [at(0., 0., 50., 16.), at(65., 0., 40., 16.), at(0., 24., 30., 16.)];
    let found = marks(&[None, DOT, DOT], &bounds);
    assert_eq!(origins(&found), vec![(56., 7.)]);
}

#[test]
fn a_part_without_a_separator_gets_none() {
    let bounds = [at(0., 0., 50., 16.), at(60., 0., 40., 16.), at(110., 0., 30., 16.)];
    let found = marks(&[None, DOT, None], &bounds);
    assert_eq!(found.len(), 1);
}

#[test]
fn the_first_part_never_shows_a_separator() {
    let bounds = [at(0., 0., 50., 16.), at(65., 0., 40., 16.)];
    assert_eq!(marks(&[DOT, None], &bounds).len(), 0);
}

#[test]
fn a_separator_sits_on_the_line_centre_between_parts_of_different_heights() {
    let bounds = [at(0., 0., 50., 36.), at(71., 8., 40., 20.)];
    let found = marks(&[None, LINE], &bounds);
    let (_, line) = found[0];
    assert!((f32::from(line.center().y) - 18.).abs() <= 0.5);
    assert!((f32::from(line.size.height) - 18.).abs() < f32::EPSILON);
}

#[test]
fn parts_in_the_next_line_do_not_overlap_vertically_with_the_one_before() {
    let bounds = [at(0., 0., 50., 16.), at(70., 16., 40., 16.)];
    assert_eq!(marks(&[None, DOT], &bounds).len(), 0);
}

#[test]
fn parts_without_bounds_get_no_separator() {
    let bounds = [at(0., 0., 50., 16.), at(65., 0., 40., 16.)];
    assert_eq!(marks(&[None, DOT, DOT], &bounds).len(), 1);
    assert_eq!(marks(&[None, DOT, DOT], &bounds[..1]).len(), 0);
    assert_eq!(marks(&[None, DOT, DOT], &[]).len(), 0);
}

#[test]
fn the_part_before_a_separator_keeps_its_room() {
    assert_eq!(rooms(&[None, DOT, DOT], 6.), vec![9., 9., 0.]);
    assert_eq!(rooms(&[None, None, LINE], 8.), vec![0., 13., 0.]);
    assert_eq!(rooms(&[None, DOT, None], 6.), vec![9., 0., 0.]);
}

#[test]
fn the_room_keeps_the_old_spacing_between_neighbours() {
    assert!((rooms(&[None, DOT], 6.)[0] + 6. - 15.).abs() < f32::EPSILON);
    assert!((rooms(&[None, LINE], 8.)[0] + 8. - 21.).abs() < f32::EPSILON);
}

#[test]
fn a_separator_is_drawn_on_whole_points() {
    let bounds = [at(0., 0., 50.3, 15.), at(64.9, 0.4, 40., 15.)];
    let found = marks(&[None, LINE], &bounds);
    for (x, y) in origins(&found) {
        assert!((x - x.round()).abs() < f32::EPSILON && (y - y.round()).abs() < f32::EPSILON);
    }
}
