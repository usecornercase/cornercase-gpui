use super::{ABOUT_ROW, layout};

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

#[test]
fn the_plan_takes_what_it_needs_between_its_limits() {
    let full = layout(1050., 536.);
    assert!(near(full.plan.expect("two columns"), 606.));
    assert!(near(full.form, 444.));
    assert!(near(layout(1050., 316.).plan.expect("two columns"), 441.));
    assert!(near(layout(1050., 900.).plan.expect("two columns"), 609.));
}

#[test]
fn a_narrow_editor_has_one_column() {
    let narrow = layout(999., 316.);
    assert_eq!(narrow.plan, None);
    assert!(near(narrow.form, 999.));
    assert!(layout(1000., 316.).plan.is_some());
}

#[test]
fn name_and_description_share_a_row_from_560_px() {
    assert!(!layout(1050., 536.).about_in_a_row());
    assert!(layout(1050., 316.).about_in_a_row());
    assert!(layout(738., 316.).about_in_a_row());
    assert!(near(ABOUT_ROW, 560.));
}
