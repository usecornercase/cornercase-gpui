use super::*;
use crate::cli::Version;
use crate::model::bots::{Kind, history_of};
use crate::test_util::fixture;

fn history() -> Vec<Line> {
    history_of(fixture!("bots/history-follow.json")).expect("the fixture")
}

fn by_id(lines: &[Line], id: u64) -> &Line {
    lines.iter().find(|line| line.id == id).expect("the line")
}

#[test]
fn the_three_kinds_are_read_with_their_agent_and_body() {
    let lines = history();
    let started = by_id(&lines, 3);
    assert_eq!((&started.kind, started.text.as_str()), (&Kind::Started, "▶ started claude in proj"));
    assert_eq!(started.agent, Some(AgentRef { pane: 6, boot: "b-1".into() }));
    assert_eq!(started.body, None);
    let finished = by_id(&lines, 5);
    assert_eq!(finished.kind, Kind::Finished);
    assert!(finished.body.as_deref().is_some_and(|body| body.ends_with("All green now.")));
    let waiting = by_id(&lines, 12);
    assert_eq!(waiting.kind, Kind::Waiting);
    assert_eq!(waiting.agent.as_ref().map(|agent| agent.pane), Some(8));
    assert!(
        waiting.body.as_deref().is_some_and(|body| body.ends_with("Do you want to run the tests?\n> 1. Yes\n  2. No"))
    );
    assert!(by_id(&lines, 17).text.starts_with('✗'));
}

#[test]
fn an_agent_without_a_pane_or_a_boot_is_none_and_the_line_stays() {
    let lines = history();
    let odd = by_id(&lines, 50);
    assert_eq!(odd.kind, Kind::Started);
    assert_eq!(odd.agent, None);
    for agent in [json(r#"{"pane": 3}"#), json(r#"{"boot": "b"}"#), json(r#""pane 3""#)] {
        assert_eq!(agent_of(&serde_json::json!({ "agent": agent })), None);
    }
}

fn json(text: &str) -> Value {
    serde_json::from_str(text).expect("json")
}

#[test]
fn a_body_that_is_not_text_is_none() {
    let line = crate::model::bots::line_of(&serde_json::json!({"id": 1, "kind": "finished", "text": "✓ x", "body": 4}))
        .expect("the line stays");
    assert_eq!(line.body, None);
}

#[test]
fn the_words_leave_the_glyph_out() {
    assert_eq!(words("▶ started claude in cornercase › fix/ci-482"), "started claude in cornercase › fix/ci-482");
    assert_eq!(words("✓ claude finished in demo"), "claude finished in demo");
    assert_eq!(words("✗ claude failed in demo"), "claude failed in demo");
    assert_eq!(words("! claude needs you in demo"), "claude needs you in demo");
    assert_eq!(words("claude in demo"), "claude in demo");
    assert_eq!(words(""), "");
}

#[test]
fn a_failed_finish_starts_with_its_cross() {
    assert!(failed("✗ claude failed in demo"));
    assert!(!failed("✓ claude finished in demo"));
    assert!(!failed("claude ✗"));
}

#[test]
fn a_pane_of_this_boot_that_is_still_here_shows() {
    assert_eq!(showable(&history(), "b-1", |pane| [6, 8, 10].contains(&pane)), vec![3, 5, 10, 12, 15, 17]);
}

#[test]
fn a_pane_id_from_another_boot_reads_as_closed() {
    assert_eq!(showable(&history(), "b-2", |_| true), Vec::<u64>::new());
    assert_eq!(showable(&history(), "", |_| true), Vec::<u64>::new());
}

#[test]
fn a_closed_pane_reads_as_closed() {
    assert_eq!(showable(&history(), "b-1", |pane| pane == 8), vec![10, 12]);
}

#[test]
fn by_names_the_bot() {
    assert_eq!(by("PRs"), "by PRs");
}

#[test]
fn the_follow_row_hides_on_an_older_server_and_says_what_this_computer_lacks() {
    let old = Some(Version::new(0, 13, 90));
    assert_eq!(follow_row(Some(Lacks::Server(old)), Os::Mac), FollowRow::Hidden);
    assert_eq!(follow_row(Some(Lacks::Machine("vps", old)), Os::Mac), FollowRow::Hidden);
    assert_eq!(
        follow_row(Some(Lacks::ThisComputer(old)), Os::Mac),
        FollowRow::Lacks("Needs cornercase 0.13.91 or newer on this Mac (it has 0.13.90).".into())
    );
    assert_eq!(follow_row(None, Os::Mac), FollowRow::Box);
}

#[test]
fn follow_is_sent_only_while_the_box_shows() {
    assert_eq!(follow_sent(&FollowRow::Box, true), Some(true));
    assert_eq!(follow_sent(&FollowRow::Box, false), Some(false));
    assert_eq!(follow_sent(&FollowRow::Hidden, true), None);
    assert_eq!(follow_sent(&FollowRow::Lacks("x".into()), false), None);
}

#[test]
fn the_note_says_what_following_does_either_way() {
    assert_ne!(follow_note(true), follow_note(false));
    assert!(follow_note(false).contains("still say it started them"));
}
