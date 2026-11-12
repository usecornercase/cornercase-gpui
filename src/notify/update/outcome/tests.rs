use serde_json::Value;

use super::*;
use crate::test_util::fixture;

fn answer(code: i32, stdout: &str, stderr: &str) -> Answer {
    Answer { code: Some(code), stdout: stdout.into(), stderr: stderr.into() }
}

fn here(version: Option<&str>) -> Here {
    Here { path: "~/x/cornercase".into(), version: version.and_then(Version::parse) }
}

fn app() -> AppState {
    AppState::from_value(&serde_json::from_str::<Value>(fixture!("app/snapshot.json")).expect("the fixture is JSON"))
}

fn expected(version: &str, boot: &str) -> Expected {
    Expected { version: version.into(), boot: boot.into() }
}

#[test]
fn an_update_that_put_the_release_in_place_is_installed() {
    let outcome =
        of(Ok(answer(0, "updated cornercase 0.13.39 → 0.13.50", "")), "0.13.50", None, Some(&here(Some("0.13.50"))));
    assert_eq!(outcome, Outcome::Installed("0.13.50".into()));
    assert_eq!(outcome.state(), InstallState::Done("0.13.50".into()));
    assert_eq!((outcome.toast("0.13.50"), outcome.waits()), (None, None));
}

#[test]
fn a_newer_cornercase_on_disk_is_installed_under_its_own_version() {
    let outcome = of(Ok(answer(0, "", "")), "0.13.50", None, Some(&here(Some("0.13.51"))));
    assert_eq!(outcome, Outcome::Installed("0.13.51".into()));
}

#[test]
fn an_update_that_left_the_old_binary_is_a_failure_naming_it() {
    let outcome = of(Ok(answer(0, "updated", "")), "0.13.50", None, Some(&here(Some("0.13.39"))));
    let why = "cornercase update finished, but ~/x/cornercase is still 0.13.39";
    assert_eq!(outcome, Outcome::Failed { why: why.into(), printed: false });
    let toast = outcome.toast("0.13.50").expect("a toast");
    assert_eq!(toast.text, format!("Could not install cornercase 0.13.50: {why}"));
    assert_eq!((toast.kind, toast.details), (ToastKind::Error, Some(Updating::Cornercase)));
}

#[test]
fn a_binary_that_does_not_say_its_version_is_a_failure() {
    let outcome = of(Ok(answer(0, "", "")), "0.13.50", None, Some(&here(None)));
    let why = "cornercase update finished, but ~/x/cornercase does not say its version";
    assert_eq!(outcome, Outcome::Failed { why: why.into(), printed: false });
}

#[test]
fn an_install_on_another_machine_is_installed_but_not_checked() {
    let outcome = of(Ok(answer(0, "", "")), "0.13.50", Some("vps"), None);
    assert_eq!(outcome, Outcome::Unchecked { version: "0.13.50".into(), host: "vps".into() });
    assert_eq!(outcome.state(), InstallState::Done("0.13.50".into()));
    let waits = outcome.waits().expect("a line");
    assert!(waits.contains("vps") && waits.contains(UPDATE_JSON), "{waits}");
}

#[test]
fn a_manual_answer_shows_the_command_not_failed() {
    let outcome = of(Ok(answer(1, fixture!("update/manual.txt"), "")), "0.13.50", None, None);
    let Outcome::Failed { why, printed } = &outcome else { panic!("not a failure: {outcome:?}") };
    assert!(why.contains("brew update && brew upgrade cornercase"), "{why}");
    assert_ne!(why, "failed");
    assert!(printed);
    assert_eq!(outcome.waits(), Some(format!("Copy command needs {UPDATE_JSON}")));
}

#[test]
fn a_failure_on_standard_error_gives_its_reason() {
    let outcome = of(Ok(answer(1, "", fixture!("update/failed.txt"))), "0.13.50", None, None);
    let why = "GitHub answered 503 to the update check";
    assert_eq!(outcome, Outcome::Failed { why: why.into(), printed: false });
    assert_eq!(outcome.waits(), None);
}

#[test]
fn a_cornercase_that_cannot_start_is_a_failure() {
    let outcome = of(Err(anyhow::anyhow!("could not run `cornercase`")), "0.13.50", None, None);
    assert!(matches!(outcome, Outcome::Failed { printed: false, .. }), "{outcome:?}");
}

#[test]
fn the_same_boot_says_nothing() {
    assert_eq!(came_back(&expected("9.0.0", "4242-1760600000"), &app()), None);
}

#[test]
fn an_app_without_a_boot_says_nothing() {
    assert_eq!(came_back(&expected("0.12.141", "x"), &AppState::default()), None);
}

#[test]
fn a_new_boot_on_the_expected_version_says_it_took() {
    let back = came_back(&expected("0.12.141", "old-boot"), &app()).expect("a verdict");
    assert_eq!(back.toast(&Where::Here(None)).text, "cornercase is now 0.12.141");
    let there = back.toast(&Where::There("vps"));
    assert_eq!((there.text.as_str(), there.kind), ("cornercase on vps is now 0.12.141", ToastKind::Done));
}

#[test]
fn a_new_boot_on_a_newer_version_also_took() {
    let back = came_back(&expected("0.12.9", "old-boot"), &app()).expect("a verdict");
    assert_eq!(back, CameBack::Now("0.12.141".into()));
}

#[test]
fn a_new_boot_on_an_older_version_names_the_binary() {
    let back = came_back(&expected("9.0.0", "old-boot"), &app()).expect("a verdict");
    let with = back.toast(&Where::Here(Some(&here(Some("9.0.0")))));
    assert_eq!(
        with.text,
        "The server came back on cornercase 0.12.141, not 9.0.0; this window runs ~/x/cornercase, which is 9.0.0"
    );
    assert_eq!(with.kind, ToastKind::Error);
    let without = back.toast(&Where::Here(Some(&here(None))));
    assert_eq!(without.text, "The server came back on cornercase 0.12.141, not 9.0.0; this window runs ~/x/cornercase");
    assert_eq!(back.toast(&Where::Here(None)).text, "The server came back on cornercase 0.12.141, not 9.0.0");
}

#[test]
fn another_machine_is_named_instead_of_the_binary() {
    let back = came_back(&expected("9.0.0", "old-boot"), &app()).expect("a verdict");
    assert_eq!(back.toast(&Where::There("vps")).text, "The server on vps came back on cornercase 0.12.141, not 9.0.0");
}
