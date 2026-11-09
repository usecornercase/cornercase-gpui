use serde_json::json;

use super::*;
use crate::test_util::fixture;

fn found(version: &str, on_disk: bool) -> Found {
    let answer: Value = serde_json::from_str(fixture!("releases/latest.json")).expect("the fixture is JSON");
    let mut latest = release::parse(&answer).expect("a release");
    latest.version = version.to_string();
    Found { latest, notes: Vec::new(), way: Way::Command("cargo install".into()), on_disk, changelog: true }
}

fn updater(found: Option<Found>, install: InstallState) -> Updater {
    Updater { found, install, ..Updater::new("https://example.test".into(), None) }
}

fn chip(found: Option<Found>, install: InstallState) -> Option<ChipState> {
    updater(found, install).chip_state()
}

#[test]
fn there_is_no_chip_without_a_release() {
    assert_eq!(chip(None, InstallState::Idle), None);
}

#[test]
fn the_chip_says_what_is_out() {
    let state = chip(Some(found("0.2.0", false)), InstallState::Idle).expect("a chip");
    assert_eq!(state.label(), "↑ cornercase-gpui 0.2.0");
    assert!(state.tip().contains("0.2.0 is out"));
}

#[test]
fn the_chip_spins_while_installing() {
    let state = chip(Some(found("0.2.0", false)), InstallState::Running).expect("a chip");
    assert_eq!(state, ChipState::Installing("0.2.0".into()));
    assert_eq!(state.label(), "installing cornercase-gpui 0.2.0…");
}

#[test]
fn the_chip_offers_the_restart_once_installed() {
    let state = chip(Some(found("0.2.0", false)), InstallState::Done("0.2.0".into())).expect("a chip");
    assert_eq!(state.label(), "↻ restart cornercase-gpui for 0.2.0");
    assert!(state.tip().contains(&format!("this one runs {CURRENT} until it restarts")));
}

#[test]
fn a_copy_installed_by_another_window_offers_the_restart() {
    let state = chip(Some(found("0.2.0", true)), InstallState::Idle).expect("a chip");
    assert_eq!(state, ChipState::Installed("0.2.0".into()));
}

#[test]
fn a_failed_install_goes_back_to_the_release() {
    let state = chip(Some(found("0.2.0", false)), InstallState::Failed("no".into()));
    assert_eq!(state, Some(ChipState::Found("0.2.0".into())));
}

fn setting(value: &Value) -> Value {
    json!({ "settings": [{ "key": "check_updates", "type": "switch", "value": value }] })
}

#[test]
fn check_updates_off_skips_the_check() {
    assert!(!wants_check(Some(&setting(&json!(false)))));
    assert!(wants_check(Some(&setting(&json!(true)))));
}

#[test]
fn an_unreadable_config_still_checks() {
    assert!(wants_check(None));
    assert!(wants_check(Some(&json!({ "settings": [] }))));
    assert!(wants_check(Some(&json!({ "nothing": 1 }))));
}

#[test]
fn release_builds_ask_github_and_only_debug_builds_ask_another_url() {
    assert_eq!(releases_url(None, true), None);
    assert_eq!(releases_url(None, false).as_deref(), Some(RELEASES_URL));
    assert_eq!(releases_url(Some("http://127.0.0.1:8765/x".into()), true).as_deref(), Some("http://127.0.0.1:8765/x"));
    assert_eq!(releases_url(Some("http://127.0.0.1:8765/x".into()), false).as_deref(), Some(RELEASES_URL));
    assert_eq!(releases_url(Some("  ".into()), true), None);
}
