use anyhow::anyhow;

use super::*;
use crate::model::Check;
use crate::test_util::fixture;

const GITHUB: &str = fixture!("pull_request/github.json");
const NONE: &str = fixture!("pull_request/none.json");

fn view() -> PullRequest {
    PullRequest {
        number: 482,
        url: "https://github.com/acme/web-shop/pull/482".into(),
        state: "open".into(),
        checks: Some("failing".into()),
        failed: vec!["tests (ubuntu-24.04)".into()],
        ..Default::default()
    }
}

fn found() -> Option<Answer> {
    answer_of(4, Ok(GITHUB.to_string()))
}

fn failed(why: &str) -> Answer {
    Answer::Failed(why.to_string())
}

fn filled(reads: &Reads) -> Option<usize> {
    let mut pr = view();
    reads.fill(&mut pr);
    pr.all_checks.map(|all| all.len())
}

#[test]
fn the_answer_fills_in_every_check_of_the_same_pull_request() {
    let mut reads = Reads::default();
    let asked = reads.start(&view());
    assert!(reads.settle(asked, found()));
    assert_eq!(filled(&reads), Some(5));
    assert_eq!(reads.note(), None);
}

#[test]
fn the_checks_of_another_pull_request_are_not_shown() {
    let mut reads = Reads::default();
    let asked = reads.start(&view());
    reads.settle(asked, found());
    let mut other = PullRequest { number: 490, url: "https://github.com/acme/web-shop/pull/490".into(), ..view() };
    reads.fill(&mut other);
    assert_eq!(other.all_checks, None);
}

#[test]
fn an_answer_older_than_the_one_shown_is_dropped() {
    let mut reads = Reads::default();
    let first = reads.start(&view());
    let second = reads.start(&PullRequest { checks: Some("pending".into()), ..view() });
    assert!(reads.settle(second, found()));
    assert!(!reads.settle(first, Some(failed("gh is not logged in: run gh auth login"))));
    assert_eq!((filled(&reads), reads.note()), (Some(5), None));
}

#[test]
fn an_older_answer_still_shows_until_the_newest_comes() {
    let mut reads = Reads::default();
    let first = reads.start(&view());
    let second = reads.start(&PullRequest { checks: Some("pending".into()), ..view() });
    assert!(reads.settle(first, found()));
    assert_eq!((filled(&reads), reads.note()), (Some(5), Some(Note::Asking)));
    assert!(reads.settle(second, found()));
    assert_eq!(reads.note(), None);
}

#[test]
fn an_answer_never_asked_for_is_dropped() {
    let mut reads = Reads::default();
    assert!(!reads.settle(1, found()));
    assert_eq!(filled(&reads), None);
}

#[test]
fn a_change_in_the_view_asks_again_and_the_same_view_does_not() {
    let mut reads = Reads::default();
    assert!(reads.due(&view()));
    reads.start(&view());
    assert!(!reads.due(&view()));
    assert!(reads.due(&PullRequest { checks: Some("passing".into()), failed: Vec::new(), ..view() }));
}

#[test]
fn a_server_too_old_is_not_asked_again() {
    let mut reads = Reads::default();
    let asked = reads.start(&view());
    reads.settle(asked, Some(Answer::TooOld));
    assert!(!reads.due(&PullRequest { checks: Some("passing".into()), ..view() }));
    assert_eq!(reads.note(), Some(Note::Said(TOO_OLD)));
}

#[test]
fn a_failed_read_says_why_and_keeps_the_views_checks() {
    let mut reads = Reads::default();
    let first = reads.start(&view());
    reads.settle(first, found());
    let second = reads.start(&PullRequest { checks: Some("pending".into()), ..view() });
    reads.settle(
        second,
        answer_of(4, Err(anyhow!("cornercase pull-request: error: gh is not logged in: run gh auth login"))),
    );
    assert_eq!(reads.note(), Some(Note::Failed("gh is not logged in: run gh auth login")));
    assert_eq!(filled(&reads), None);
}

#[test]
fn an_older_server_or_command_line_reads_as_too_old() {
    let server = "cornercase pull-request: error: the running cornercase server (0.12.236) has no `pull-request` \
                  command; update cornercase and restart the server";
    let client =
        "cornercase pull-request: error: unrecognized subcommand 'pull-request'\n\nUsage: cornercase <COMMAND>";
    assert_eq!(answer_of(4, Err(anyhow!(server))), Some(Answer::TooOld));
    assert_eq!(answer_of(4, Err(anyhow!(client))), Some(Answer::TooOld));
    assert_eq!(answer_of(4, Err(anyhow!("cornercase pull-request: no workspace 9"))), Some(failed("no workspace 9")));
}

#[test]
fn no_pull_request_says_the_reason_given() {
    assert_eq!(answer_of(4, Ok(NONE.to_string())), Some(Answer::Missing("no pull request for feat/x".into())));
}

#[test]
fn an_answer_about_another_workspace_is_ignored_but_ends_the_wait() {
    let mut reads = Reads::default();
    let asked = reads.start(&view());
    assert_eq!(answer_of(5, Ok(GITHUB.to_string())), None);
    assert!(reads.settle(asked, answer_of(5, Ok(GITHUB.to_string()))));
    assert_eq!((filled(&reads), reads.note()), (None, None));
}

#[test]
fn asking_shows_until_an_answer_comes() {
    let mut reads = Reads::default();
    assert_eq!(reads.note(), None);
    reads.start(&view());
    assert_eq!(reads.note(), Some(Note::Asking));
}

#[test]
fn every_check_comes_from_the_answer_not_the_view() {
    let mut reads = Reads::default();
    let asked = reads.start(&view());
    reads.settle(asked, found());
    let mut pr = view();
    reads.fill(&mut pr);
    let first = pr.all_checks.as_deref().and_then(<[Check]>::first).map(|c| c.url.as_str());
    assert_eq!(first, Some("https://github.com/acme/web-shop/actions/runs/1001/job/2001"));
}

#[test]
fn it_asks_for_the_workspace_with_json() {
    assert_eq!(args(4), ["pull-request", "--workspace", "4", "--json"]);
}

fn answered_with(outcomes: &[&str]) -> Reads {
    let all =
        outcomes.iter().map(|o| Check { name: (*o).into(), outcome: (*o).into(), ..Default::default() }).collect();
    let mut reads = Reads::default();
    let asked = reads.start(&view());
    reads.settle(asked, Some(Answer::Found(Box::new(PullRequest { all_checks: Some(all), ..view() }))));
    reads
}

#[test]
fn a_pending_check_is_asked_about_again() {
    assert!(answered_with(&["failed", "pending", "pending"]).refresh_due());
    let mut reads = Reads::default();
    let asked = reads.start(&view());
    reads.settle(asked, found());
    assert!(reads.refresh_due());
}

#[test]
fn once_nothing_is_pending_it_is_not_asked_about_again() {
    assert!(!answered_with(&["failed", "passed", "skipped"]).refresh_due());
    assert!(!answered_with(&[]).refresh_due());
}

#[test]
fn no_refresh_is_due_while_a_read_runs() {
    let mut reads = answered_with(&["pending"]);
    reads.start(&view());
    assert!(!reads.refresh_due());
}

#[test]
fn a_failed_read_an_older_server_or_no_answer_is_not_asked_about_again() {
    let mut reads = answered_with(&["pending"]);
    let asked = reads.start(&view());
    reads.settle(asked, Some(failed("gh is not logged in: run gh auth login")));
    assert!(!reads.refresh_due());
    let asked = reads.start(&view());
    reads.settle(asked, Some(Answer::TooOld));
    assert!(!reads.refresh_due());
    assert!(!Reads::default().refresh_due());
}

#[test]
fn a_refresh_asks_again_about_the_same_view() {
    let mut reads = answered_with(&["pending"]);
    assert!(!reads.due(&view()));
    let again = reads.start(&view());
    assert!(reads.asking());
    assert!(reads.settle(again, found()));
}
