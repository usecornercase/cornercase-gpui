use super::*;

#[test]
fn the_browser_opens_on_half_a_window() {
    assert!((wide_default(1200.) - 600.).abs() < f32::EPSILON);
}

#[test]
fn the_browser_opens_no_wider_than_its_default_on_a_big_window() {
    assert!((wide_default(2560.) - WIDE_DOCK.size()).abs() < f32::EPSILON);
}

#[test]
fn the_browser_and_the_flow_panel_keep_widths_of_their_own() {
    assert!(DOCKS.wide(PanelKind::Browser).is_some());
    assert!(DOCKS.wide(PanelKind::Flow).is_some());
    assert!(DOCKS.wide(PanelKind::Changes).is_none());
    assert!(DOCKS.wide(PanelKind::Todo).is_none());
}

#[test]
fn the_browser_opens_at_least_as_wide_as_the_other_docked_panels() {
    assert!((wide_default(700.) - DOCK.size()).abs() < f32::EPSILON);
}
