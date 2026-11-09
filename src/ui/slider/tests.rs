use super::*;

#[test]
fn a_press_at_a_quarter_gives_a_quarter_of_the_range() {
    assert!((value_at(0.25, (0., 1.), 0.) - 0.25).abs() < 1e-4);
    assert!((value_at(0.25, (0.55, 1.), 0.) - 0.6625).abs() < 1e-4);
}

#[test]
fn values_snap_to_their_step() {
    assert!((value_at(0.33, (0., 1.), 0.05) - 0.35).abs() < 1e-4);
}

#[test]
fn a_drag_past_the_ends_stays_within_the_range() {
    assert!((value_at(-3., (0.3, 0.9), 0.01) - 0.3).abs() < 1e-4);
    assert!((value_at(9., (0.3, 0.9), 0.01) - 0.9).abs() < 1e-4);
}

#[test]
fn a_value_sits_at_its_fraction_of_the_range() {
    assert!((fraction_of(0.775, (0.55, 1.)) - 0.5).abs() < 1e-4);
    assert!(fraction_of(5., (0., 1.)).abs() - 1. < 1e-4);
    assert!(fraction_of(1., (1., 1.)).abs() < 1e-4);
}
