use serde_json::Value;

use super::*;
use crate::test_util::fixture;

#[expect(clippy::unnecessary_wraps, reason = "the readers take the Result a command gives")]
fn answer(code: i32, stdout: &str, stderr: &str) -> anyhow::Result<Answer> {
    Ok(Answer { code: Some(code), stdout: stdout.into(), stderr: stderr.into() })
}

fn release(text: &str) -> Option<Version> {
    Version::parse(text)
}

fn through(here: &str, there: &str) -> UpdateGate {
    UpdateGate::Through { here: release(here), there: release(there) }
}

fn by_hand() -> ByHand {
    ByHand {
        version: "0.13.53".into(),
        latest: "0.13.74".into(),
        command: "brew update && brew upgrade cornercase".into(),
    }
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
fn an_updated_line_is_installed_under_its_version() {
    let (place, gate) = (Where::Here(None), UpdateGate::Here(release("0.13.74")));
    let outcome = of(&answer(0, fixture!("update/json-updated.txt"), ""), "0.13.74", &place);
    assert_eq!(outcome, Outcome::Installed("0.13.74".into()));
    assert_eq!(outcome.state(), InstallState::Done("0.13.74".into()));
    assert_eq!((outcome.toast("0.13.74", &place), outcome.waits(gate, &place, Os::Mac)), (None, None));
}

#[test]
fn an_updated_line_below_the_release_is_installed_as_it_is() {
    let outcome = of(&answer(0, fixture!("update/json-updated.txt"), ""), "0.13.75", &Where::Here(None));
    assert_eq!(outcome, Outcome::Installed("0.13.74".into()));
}

#[test]
fn an_updated_line_is_not_checked_against_the_binary_on_disk() {
    let place = Where::Here(Some(&here(Some("0.13.50"))));
    let outcome = of(&answer(0, fixture!("update/json-updated.txt"), ""), "0.13.74", &place);
    assert_eq!(outcome, Outcome::Installed("0.13.74".into()));
}

#[test]
fn up_to_date_at_the_release_offers_the_restart() {
    let line = fixture!("update/json-up-to-date.txt");
    let at = of(&answer(0, line, ""), "0.13.74", &Where::Here(None));
    assert_eq!(at, Outcome::Installed("0.13.74".into()));
    let below = of(&answer(0, line, ""), "0.13.73", &Where::Here(None));
    assert_eq!(below, Outcome::Installed("0.13.74".into()));
}

#[test]
fn up_to_date_below_the_release_is_a_failure_naming_it() {
    let line = fixture!("update/json-up-to-date.txt");
    let here = of(&answer(0, line, ""), "0.13.75", &Where::Here(None));
    let why = "cornercase update found no release newer than 0.13.74";
    assert_eq!(here, Outcome::Failed { why: why.into(), printed: false });
    let toast = here.toast("0.13.75", &Where::Here(None)).expect("a toast");
    assert_eq!(toast.text, format!("Could not install cornercase 0.13.75: {why}"));
    assert_eq!((toast.kind, toast.details), (ToastKind::Error, Some(Updating::Cornercase)));
    let there = of(&answer(0, line, ""), "0.13.75", &Where::There("vps"));
    let why = "cornercase update on vps found no release newer than 0.13.74";
    assert_eq!(there, Outcome::Failed { why: why.into(), printed: false });
}

#[test]
fn a_manual_line_is_by_hand_with_its_command() {
    let line = fixture!("update/json-manual.txt");
    let outcome = of(&answer(6, line, ""), "0.13.74", &Where::Here(None));
    assert_eq!(outcome, Outcome::Manual(by_hand()));
    assert_eq!(outcome.state(), InstallState::Manual(by_hand()));
    let toast = outcome.toast("0.13.74", &Where::Here(None)).expect("a toast");
    assert_eq!(toast.text, "To install cornercase 0.13.74, run the command in Details");
    assert_eq!((toast.kind, toast.details), (ToastKind::NeedsYou, Some(Updating::Cornercase)));
    let there = outcome.toast("0.13.74", &Where::There("vps")).expect("a toast");
    assert_eq!(there.text, "To install cornercase 0.13.74 on vps, run the command in Details on vps");
    assert_eq!(there.kind, ToastKind::NeedsYou);
}

#[test]
fn a_refusal_is_a_failure_in_the_windows_words() {
    let outcome = of(&answer(2, "", fixture!("update/json-old.txt")), "0.13.74", &Where::Here(None));
    let why = "cornercase refused update --json, which needs cornercase 0.13.53 or newer";
    assert_eq!(outcome, Outcome::Failed { why: why.into(), printed: false });
    let toast = outcome.toast("0.13.74", &Where::Here(None)).expect("a toast");
    assert_eq!(toast.text, format!("Could not install cornercase 0.13.74: {why}"));
}

#[test]
fn an_install_on_another_machine_is_checked_by_its_line() {
    let place = Where::There("vps");
    let outcome = of(&answer(0, fixture!("update/json-updated.txt"), ""), "0.13.74", &place);
    assert_eq!(outcome, Outcome::Installed("0.13.74".into()));
    assert_eq!(outcome.waits(through("0.13.74", "0.13.53"), &place, Os::Mac), None);
}

#[test]
fn an_update_that_put_the_release_in_place_is_installed() {
    let place = Where::Here(Some(&here(Some("0.13.50"))));
    let outcome = of(&answer(0, "updated cornercase 0.13.39 → 0.13.50", ""), "0.13.50", &place);
    assert_eq!(outcome, Outcome::Installed("0.13.50".into()));
}

#[test]
fn a_newer_cornercase_on_disk_is_installed_under_its_own_version() {
    let place = Where::Here(Some(&here(Some("0.13.51"))));
    assert_eq!(of(&answer(0, "", ""), "0.13.50", &place), Outcome::Installed("0.13.51".into()));
}

#[test]
fn an_update_that_left_the_old_binary_is_a_failure_naming_it() {
    let place = Where::Here(Some(&here(Some("0.13.39"))));
    let outcome = of(&answer(0, "updated", ""), "0.13.50", &place);
    let why = "cornercase update finished, but ~/x/cornercase is still 0.13.39";
    assert_eq!(outcome, Outcome::Failed { why: why.into(), printed: false });
    let toast = outcome.toast("0.13.50", &place).expect("a toast");
    assert_eq!(toast.text, format!("Could not install cornercase 0.13.50: {why}"));
    assert_eq!((toast.kind, toast.details), (ToastKind::Error, Some(Updating::Cornercase)));
}

#[test]
fn a_binary_that_does_not_say_its_version_is_a_failure() {
    let place = Where::Here(Some(&here(None)));
    let outcome = of(&answer(0, "", ""), "0.13.50", &place);
    let why = "cornercase update finished, but ~/x/cornercase does not say its version";
    assert_eq!(outcome, Outcome::Failed { why: why.into(), printed: false });
}

#[test]
fn an_install_there_below_the_gate_is_not_checked_and_says_what_it_needs() {
    let place = Where::There("vps");
    let outcome = of(&answer(0, "", ""), "0.13.50", &place);
    assert_eq!(outcome, Outcome::Unchecked { version: "0.13.50".into(), host: "vps".into() });
    assert_eq!(outcome.state(), InstallState::Done("0.13.50".into()));
    let waits = |gate| outcome.waits(gate, &place, Os::Mac);
    let said = "Not checked on vps until its server restarts";
    assert_eq!(waits(through("0.13.74", "0.13.51")), Some(format!("{said}: needs cornercase 0.13.53 or newer on vps")));
    assert_eq!(
        waits(through("0.13.51", "0.13.74")),
        Some(format!("{said}: needs cornercase 0.13.53 or newer on this Mac (it has 0.13.51)"))
    );
    assert_eq!(
        waits(through("0.13.51", "0.13.50")),
        Some(format!("{said}: needs cornercase 0.13.53 or newer on this Mac (it has 0.13.51) and on vps"))
    );
    assert_eq!(waits(through("0.13.74", "0.13.53")), Some(said.to_string()));
}

#[test]
fn a_manual_answer_below_the_gate_names_the_release_copy_command_needs() {
    let place = Where::Here(None);
    let outcome = of(&answer(1, fixture!("update/manual.txt"), ""), "0.13.50", &place);
    let Outcome::Failed { why, printed } = &outcome else { panic!("not a failure: {outcome:?}") };
    assert!(why.contains("brew update && brew upgrade cornercase"), "{why}");
    assert!(printed);
    let waits = outcome.waits(UpdateGate::Here(release("0.13.51")), &place, Os::Mac);
    assert_eq!(waits.as_deref(), Some("Copy command needs cornercase 0.13.53 or newer on this Mac (it has 0.13.51)"));
    assert_eq!(outcome.waits(UpdateGate::Here(release("0.13.74")), &place, Os::Mac), None);
}

#[test]
fn a_manual_answer_with_status_six_and_no_line_is_what_it_printed() {
    let place = Where::Here(None);
    let outcome = of(&answer(6, fixture!("update/manual-6.txt"), ""), "0.13.74", &place);
    assert!(matches!(outcome, Outcome::Failed { printed: true, .. }), "{outcome:?}");
}

#[test]
fn this_computers_release_unknown_needs_no_parenthesis() {
    let place = Where::Here(None);
    let outcome = of(&answer(1, fixture!("update/manual.txt"), ""), "0.13.50", &place);
    let waits = outcome.waits(UpdateGate::Here(None), &place, Os::Mac);
    assert_eq!(waits.as_deref(), Some("Copy command needs cornercase 0.13.53 or newer on this Mac"));
}

#[test]
fn on_linux_it_names_this_computer() {
    let place = Where::Here(None);
    let outcome = of(&answer(1, fixture!("update/manual.txt"), ""), "0.13.50", &place);
    let waits = outcome.waits(UpdateGate::Here(release("0.13.51")), &place, Os::Linux);
    assert_eq!(
        waits.as_deref(),
        Some("Copy command needs cornercase 0.13.53 or newer on this computer (it has 0.13.51)")
    );
}

#[test]
fn a_failure_on_standard_error_gives_its_reason() {
    let place = Where::Here(None);
    let outcome = of(&answer(1, "", fixture!("update/failed.txt")), "0.13.50", &place);
    let why = "GitHub answered 503 to the update check";
    assert_eq!(outcome, Outcome::Failed { why: why.into(), printed: false });
    assert_eq!(outcome.waits(UpdateGate::Here(release("0.13.51")), &place, Os::Mac), None);
}

#[test]
fn a_cornercase_that_cannot_start_is_a_failure() {
    let outcome = of(&Err(anyhow::anyhow!("could not run `cornercase`")), "0.13.50", &Where::Here(None));
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
