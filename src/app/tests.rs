use super::*;

#[test]
fn the_browser_opens_on_half_a_window() {
    assert!((browser_default(1200.) - 600.).abs() < f32::EPSILON);
}

#[test]
fn the_browser_opens_no_wider_than_its_default_on_a_big_window() {
    assert!((browser_default(2560.) - BROWSER_DOCK.size()).abs() < f32::EPSILON);
}

#[test]
fn the_browser_opens_at_least_as_wide_as_the_other_docked_panels() {
    assert!((browser_default(700.) - DOCK.size()).abs() < f32::EPSILON);
}
