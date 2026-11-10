use anyhow::anyhow;

use super::{Answer, Older, Said, Start, Started, Step, refusal, said, said_of, start, step, update_args};
use crate::test_util::fixture;

fn older() -> Older {
    refusal(Some(5), fixture!("remote/older.json")).expect("a refusal")
}

fn up(version: &str) -> Start {
    Start::Up(Started { version: version.to_string() })
}

fn answer(code: i32, stdout: &str, stderr: &str) -> Answer {
    Answer { code: Some(code), stdout: stdout.to_string(), stderr: stderr.to_string() }
}

#[test]
fn a_refusal_is_read_from_status_5_and_its_json() {
    assert_eq!(
        older(),
        Older {
            destination: "vps".into(),
            there: Some("0.13.21".into()),
            here: "0.13.28".into(),
            update: ["cornercase", "--remote", "vps", "update"].map(String::from).to_vec(),
        }
    );
}

#[test]
fn the_update_keeps_the_remote_command_as_one_word() {
    let older = refusal(Some(5), fixture!("remote/older_with_command.json")).expect("a refusal");
    assert_eq!(older.update[3..5], ["--remote-command", "env LANG=C /home/ana/bin/cornercase"]);
}

#[test]
fn a_cornercase_too_old_for_the_proxy_has_no_release() {
    let older = refusal(Some(5), fixture!("remote/too_old.json")).expect("a refusal");
    assert_eq!(older.there, None);
}

#[test]
fn status_5_without_its_json_is_not_a_refusal() {
    for stdout in ["", "not json", r#"{"error":"x"}"#] {
        assert_eq!(refusal(Some(5), stdout), None, "{stdout}");
    }
}

#[test]
fn the_json_without_status_5_is_not_a_refusal() {
    assert_eq!(refusal(Some(1), fixture!("remote/older.json")), None);
}

#[test]
fn another_error_code_is_not_a_refusal() {
    let other = fixture!("remote/older.json").replace("\"older\"", "\"stale-boot\"");
    assert_eq!(refusal(Some(5), &other), None);
}

#[test]
fn an_older_cornercase_here_fails_as_before() {
    let got = start(Some(1), b"", fixture!("remote/refused_by_0_13_21.txt").as_bytes());
    let Start::Failed(why) = got else { panic!("expected a failure") };
    assert!(why.starts_with("cornercase start-server: Error: "), "{why}");
}

#[test]
fn a_started_server_says_its_release() {
    let got = start(Some(0), fixture!("remote/started_behind.json").as_bytes(), b"");
    assert_eq!(got, up("0.13.21"));
    assert_eq!(start(Some(0), b"started", b""), Start::Up(Started::default()));
}

#[test]
fn a_refused_start_is_its_own_start() {
    let got = start(Some(5), fixture!("remote/older.json").as_bytes(), b"error: x");
    assert_eq!(got, Start::Older(older()));
}

#[test]
fn a_server_up_opens_the_view_when_nothing_was_updated() {
    assert_eq!(step(up("0.13.21"), None), Step::Open);
}

#[test]
fn after_an_update_a_server_still_older_than_here_is_behind() {
    let Step::Behind(behind) = step(up("0.13.21"), Some(&older())) else { panic!("expected behind") };
    assert_eq!(
        (behind.destination.as_str(), behind.server.as_str(), behind.here.as_str()),
        ("vps", "0.13.21", "0.13.28")
    );
}

#[test]
fn after_an_update_a_server_on_this_release_or_newer_opens() {
    let restarted = start(Some(0), fixture!("remote/started_after_restart.json").as_bytes(), b"");
    assert_eq!(step(restarted, Some(&older())), Step::Open);
    assert_eq!(step(up("0.13.30"), Some(&older())), Step::Open);
}

#[test]
fn a_server_that_does_not_say_its_release_is_not_behind() {
    assert_eq!(step(up(""), Some(&older())), Step::Open);
    assert_eq!(step(up("x"), Some(&older())), Step::Open);
}

#[test]
fn a_refusal_and_a_failure_are_their_own_steps() {
    assert_eq!(step(Start::Older(older()), None), Step::Older(older()));
    assert_eq!(step(Start::Failed("boom".into()), None), Step::Lost("boom".into()));
}

#[test]
fn the_update_runs_the_words_after_cornercase() {
    assert_eq!(update_args(&older()), Some(["--remote", "vps", "update"].map(String::from).to_vec()));
    let with = refusal(Some(5), fixture!("remote/older_with_command.json")).expect("a refusal");
    let args = update_args(&with).expect("words");
    assert_eq!(args[2..4], ["--remote-command", "env LANG=C /home/ana/bin/cornercase"]);
    assert_eq!(args.last().map(String::as_str), Some("update"));
}

#[test]
fn words_that_do_not_start_with_cornercase_offer_no_update() {
    for words in [vec![], vec!["cornercase"], vec!["sh", "-c", "x"]] {
        let older = Older { update: words.iter().map(ToString::to_string).collect(), ..older() };
        assert_eq!(update_args(&older), None, "{words:?}");
    }
}

#[test]
fn an_update_that_exits_0_is_done() {
    let text = fixture!("remote/update_done.txt");
    assert_eq!(said(&answer(0, text, "")), Said::Done(text.trim().to_string()));
}

#[test]
fn an_update_cornercase_cannot_run_says_what_to_run() {
    let Said::Failed(text) = said(&answer(1, fixture!("remote/update_by_hand.txt"), "")) else {
        panic!("expected a failure")
    };
    assert!(text.contains("brew update && brew upgrade cornercase"), "{text}");
}

#[test]
fn a_failed_update_gives_its_reason() {
    let stderr = "Error: could not reach GitHub: io: Connection refused (os error 61)";
    assert_eq!(
        said(&answer(1, "", stderr)),
        Said::Failed("could not reach GitHub: io: Connection refused (os error 61)".into())
    );
}

#[test]
fn an_update_that_could_not_start_says_so() {
    let said = said_of(Err(anyhow!("could not run `cornercase`")));
    assert_eq!(said, Said::Failed("could not run `cornercase`".into()));
}

#[test]
fn an_update_that_says_nothing_still_says_something() {
    let Said::Failed(text) = said(&answer(1, "", "")) else { panic!("expected a failure") };
    assert_ne!(text, "");
}
