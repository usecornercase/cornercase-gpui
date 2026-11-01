use super::*;

#[test]
fn a_dialog_body_grows_with_the_window_within_limits() {
    assert_eq!(body_height(800.), 560.);
    assert_eq!(body_height(600.), 360.);
    assert_eq!(body_height(300.), 200.);
    assert_eq!(body_height(1400.), 560.);
}
