use gpui::{point, px, size};

use super::*;

fn bounds() -> Bounds<Pixels> {
    Bounds::new(point(px(10.), px(20.)), size(px(100.), px(50.)))
}

#[test]
fn a_point_in_the_middle_is_half_along_each_way() {
    let (x, y) = along(bounds(), point(px(60.), px(45.)));
    assert!((x - 0.5).abs() < 1e-4 && (y - 0.5).abs() < 1e-4);
}

#[test]
fn a_point_outside_stays_at_the_nearest_edge() {
    assert_eq!(along(bounds(), point(px(-500.), px(900.))), (0., 1.));
}

#[test]
fn an_unknown_track_has_no_bounds() {
    assert!(bounds_of("never-drawn").is_none());
    assert!(pointer_along("never-drawn", point(px(0.), px(0.))).is_none());
}
