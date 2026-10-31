use serde_json::json;

use super::*;

fn with_mouse(mode: MouseMode) -> PaneGrid {
    let mut pane = PaneGrid::default();
    pane.modes.mouse = mode;
    pane
}

#[test]
fn input_goes_to_the_popup_it_was_meant_for() {
    let event = json!({"key": {"code": "down"}});
    assert_eq!(input_args(40, &event), ["input", "--popup", "--pane", "40", r#"{"key":{"code":"down"}}"#]);
}

#[test]
fn the_title_names_the_action_then_its_plugin() {
    let open = OpenPopup { plugin: "acme.notes".into(), action: "browse".into(), cols: 60, rows: 10 };
    assert_eq!(title(&open), "browse · acme.notes");
}

#[test]
fn a_stream_that_fails_the_same_way_twice_waits_longer() {
    assert_eq!(retry_after("not-found", Some("not-found")), RETRY_FAILED);
    assert_eq!(retry_after("not-found", None), RETRY);
    assert_eq!(retry_after("", Some("")), RETRY);
}

#[test]
fn only_a_program_that_reads_releases_gets_them() {
    assert!(!takes_release(&with_mouse(MouseMode::None)));
    assert!(!takes_release(&with_mouse(MouseMode::Press)));
    assert!(takes_release(&with_mouse(MouseMode::PressRelease)));
    assert!(takes_release(&with_mouse(MouseMode::AnyMotion)));
}
