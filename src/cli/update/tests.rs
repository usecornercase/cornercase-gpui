use anyhow::anyhow;

use super::{ByHand, UpdateAnswer, UpdateGate, update_answer, update_command};
use crate::cli::{Answer, Older, Said, Version};
use crate::test_util::fixture;

#[expect(clippy::unnecessary_wraps, reason = "the readers take the Result a command gives")]
fn reply(code: i32, stdout: &str, stderr: &str) -> anyhow::Result<Answer> {
    Ok(Answer { code: Some(code), stdout: stdout.to_string(), stderr: stderr.to_string() })
}

fn release(text: &str) -> Option<Version> {
    Version::parse(text)
}

fn older(here: &str, there: Option<&str>) -> Older {
    Older {
        destination: "vps.invalid".into(),
        there: there.map(str::to_string),
        here: here.into(),
        update: ["cornercase", "--remote", "vps.invalid", "update"].map(str::to_string).to_vec(),
    }
}

#[test]
fn an_updated_line_says_what_is_on_disk_and_what_it_replaced() {
    let answer = update_answer(&reply(0, fixture!("update/json-updated.txt"), ""));
    assert_eq!(answer, UpdateAnswer::Updated { from: "0.13.53".into(), version: "0.13.74".into() });
    assert!(answer.ended());
}

#[test]
fn an_up_to_date_line_names_the_binary_on_disk() {
    let answer = update_answer(&reply(0, fixture!("update/json-up-to-date.txt"), ""));
    assert_eq!(answer, UpdateAnswer::UpToDate { version: "0.13.74".into() });
    assert!(answer.ended());
}

#[test]
fn a_manual_line_gives_the_release_and_the_command() {
    let answer = update_answer(&reply(6, fixture!("update/json-manual.txt"), ""));
    let by_hand = ByHand {
        version: "0.13.53".into(),
        latest: "0.13.74".into(),
        command: "brew update && brew upgrade cornercase".into(),
    };
    assert_eq!(answer, UpdateAnswer::Manual(by_hand));
    assert!(!answer.ended());
}

#[test]
fn a_manual_line_for_a_folder_it_cannot_write_gives_the_installer() {
    let UpdateAnswer::Manual(by_hand) = update_answer(&reply(6, fixture!("update/json-manual-installer.txt"), ""))
    else {
        panic!("not a manual answer");
    };
    assert_eq!(by_hand.command, "curl -fsSL https://usecornercase.dev/install.sh | sh");
}

#[test]
fn a_line_is_read_only_with_the_status_it_comes_with() {
    let answer = update_answer(&reply(1, fixture!("update/json-updated.txt"), "Error: broke"));
    assert!(matches!(answer, UpdateAnswer::Printed(Said::Failed(_))));
}

#[test]
fn a_checks_available_line_is_no_install() {
    let answer = update_answer(&reply(0, fixture!("update/check-available.txt"), ""));
    assert!(matches!(answer, UpdateAnswer::Printed(Said::Done(_))));
}

#[test]
fn a_checks_up_to_date_line_names_the_binary_on_disk() {
    let answer = update_answer(&reply(0, fixture!("update/check-up-to-date.txt"), ""));
    assert_eq!(answer, UpdateAnswer::UpToDate { version: "0.13.74".into() });
}

#[test]
fn a_checks_manual_line_with_status_zero_is_still_read() {
    let answer = update_answer(&reply(0, fixture!("update/check-manual.txt"), ""));
    assert!(matches!(answer, UpdateAnswer::Manual(_)));
}

#[test]
fn an_unknown_outcome_is_read_by_its_status() {
    let line = "{\"outcome\":\"later\",\"version\":\"9.9.9\"}";
    assert!(matches!(update_answer(&reply(0, line, "")), UpdateAnswer::Printed(Said::Done(_))));
    assert!(matches!(update_answer(&reply(6, line, "")), UpdateAnswer::Printed(Said::Failed(_))));
}

#[test]
fn a_line_missing_a_key_is_read_by_its_status() {
    let line = "{\"outcome\":\"updated\",\"version\":\"0.13.74\"}";
    assert!(matches!(update_answer(&reply(0, line, "")), UpdateAnswer::Printed(Said::Done(_))));
}

#[test]
fn a_line_that_is_no_json_is_read_by_its_status() {
    let answer = update_answer(&reply(0, "updated cornercase 0.13.51 → 0.13.74\n", ""));
    assert_eq!(answer, UpdateAnswer::Printed(Said::Done("updated cornercase 0.13.51 → 0.13.74".into())));
}

#[test]
fn the_first_line_that_reads_is_the_answer() {
    let out = format!("Welcome to the machine\n{}", fixture!("update/json-updated.txt"));
    assert!(matches!(update_answer(&reply(0, &out, "")), UpdateAnswer::Updated { .. }));
}

#[test]
fn a_failure_with_nothing_on_stdout_gives_cornercases_reason() {
    let answer = update_answer(&reply(1, "", fixture!("update/json-failed.txt")));
    assert_eq!(answer, UpdateAnswer::Printed(Said::Failed("GitHub answered 404 to the update check".into())));
    assert!(!answer.ended());
}

#[test]
fn status_two_is_a_refusal_whatever_it_printed() {
    for stderr in [fixture!("update/json-old.txt"), fixture!("update/json-yes.txt")] {
        assert_eq!(update_answer(&reply(2, "", stderr)), UpdateAnswer::Refused);
    }
    assert_eq!(update_answer(&reply(2, fixture!("update/json-updated.txt"), "")), UpdateAnswer::Refused);
    assert!(!UpdateAnswer::Refused.ended());
}

#[test]
fn a_refusal_through_ssh_is_a_refusal_too() {
    assert_eq!(update_answer(&reply(2, "", fixture!("remote/update-json-old.txt"))), UpdateAnswer::Refused);
}

#[test]
fn a_line_through_ssh_reads_as_it_does_here() {
    let answer = update_answer(&reply(0, fixture!("remote/update-json-updated.txt"), ""));
    assert_eq!(answer, UpdateAnswer::Updated { from: "0.13.53".into(), version: "0.13.74".into() });
}

#[test]
fn status_six_without_a_line_is_what_it_printed() {
    let UpdateAnswer::Printed(Said::Failed(text)) = update_answer(&reply(6, fixture!("update/manual-6.txt"), ""))
    else {
        panic!("not what it printed");
    };
    assert!(text.contains("brew update && brew upgrade cornercase"));
}

#[test]
fn a_cornercase_that_cannot_start_is_a_failure() {
    let answer = update_answer(&Err(anyhow!("cornercase: not found")));
    assert!(matches!(answer, UpdateAnswer::Printed(Said::Failed(_))));
}

#[test]
fn this_computer_sends_json_from_the_release_that_has_it() {
    let words = || vec!["update".to_string()];
    for (here, json) in
        [(release("0.13.52"), false), (None, false), (release("0.13.53"), true), (release("0.13.74"), true)]
    {
        let sent = UpdateGate::Here(here).args(words());
        assert_eq!(sent.last().map(String::as_str) == Some("--json"), json, "{here:?}");
    }
}

#[test]
fn through_remote_both_command_lines_need_it() {
    let lacks = |here, there| UpdateGate::Through { here: release(here), there: release(there) }.lacks();
    assert!(!lacks("0.13.74", "0.13.53"));
    assert!(lacks("0.13.74", "0.13.51"));
    assert!(lacks("0.13.74", "unknown"));
    assert!(lacks("0.13.51", "0.13.74"));
    assert!(lacks("0.13.51", "0.13.51"));
    assert!(lacks("unknown", "0.13.74"));
}

#[test]
fn a_refusals_words_get_json_when_both_releases_have_it() {
    let sent = update_command(&older("0.13.74", Some("0.13.53"))).expect("words");
    assert_eq!(sent, ["--remote", "vps.invalid", "update", "--json"]);
    let plain = ["--remote", "vps.invalid", "update"];
    assert_eq!(update_command(&older("0.13.74", Some("0.13.51"))).expect("words"), plain);
    assert_eq!(update_command(&older("0.13.74", None)).expect("words"), plain);
    let mut none = older("0.13.74", Some("0.13.53"));
    none.update.clear();
    assert_eq!(update_command(&none), None);
}
