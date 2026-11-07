use serde_json::Value;

use super::{Forum, Stage, of_app, picked};
use crate::test_util::fixture;

fn feed(name: &str) -> Option<Forum> {
    let app: Value = serde_json::from_str(name).expect("the feed parses");
    of_app(Some(&app))
}

fn ready() -> Forum {
    feed(fixture!("settings/forum-ready.json")).expect("a forum")
}

fn waiting() -> Forum {
    feed(fixture!("settings/forum-pending.json")).expect("a forum")
}

fn picking() -> Forum {
    feed(fixture!("settings/forum-pick.json")).expect("a forum")
}

#[test]
fn a_feed_without_a_forum_has_none() {
    assert_eq!(feed(fixture!("settings/forum-none.json")), None);
    assert_eq!(of_app(None), None);
}

#[test]
fn the_forum_is_read_from_discords_row() {
    let telegram = r#"{"away":[{"platform":"telegram","forum":{"on":true}}]}"#;
    assert_eq!(feed(telegram), None);
}

#[test]
fn a_ready_forum_says_so_and_links_to_its_channel() {
    let forum = ready();
    assert_eq!(forum.stage(), Stage::Ready);
    assert_eq!(forum.words(), "Forum ready: each workspace gets its own post");
    assert_eq!(forum.address().as_deref(), Some("https://discord.com/channels/7/800"));
}

#[test]
fn a_forum_being_set_up_says_its_next_step_and_links_nowhere() {
    let forum = waiting();
    assert_eq!(forum.stage(), Stage::Waiting);
    assert_eq!(forum.words(), "Forum not ready: turn on Guild Install: Developer Portal › Installation");
    assert_eq!(forum.address(), None);
}

#[test]
fn a_forum_turned_on_with_nothing_left_named_is_being_set_up() {
    let forum = Forum { on: true, ..Forum::default() };
    assert_eq!(forum.words(), "Forum not ready: setting it up");
}

#[test]
fn a_forum_kept_while_off_links_nowhere() {
    let forum = Forum { ready: true, ..ready() };
    let off = Forum { on: false, ..forum };
    assert_eq!(off.stage(), Stage::Off);
    assert_eq!(off.address(), None);
}

#[test]
fn a_forum_that_waits_for_a_pick_lists_its_servers_and_says_so_in_the_tuis_words() {
    let forum = picking();
    assert_eq!(forum.stage(), Stage::Waiting);
    assert_eq!(forum.words(), "Forum not ready: pick the server for the forum");
    let names: Vec<&str> = forum.servers.iter().map(|server| server.name.as_str()).collect();
    assert_eq!(names, ["Ana's server", "Side projects"]);
}

#[test]
fn a_forum_with_no_pick_waiting_lists_no_servers() {
    assert_eq!(waiting().servers, Vec::new());
    assert_eq!(ready().servers, Vec::new());
}

#[test]
fn a_server_is_picked_by_its_id() {
    let forum = picking();
    assert_eq!(forum.servers[1].pick_args(), ["config", "forum-server", "--json", "--", "1100000000000000002"]);
}

#[test]
fn each_server_has_a_job_of_its_own() {
    let forum = picking();
    assert_ne!(forum.servers[0].picking(), forum.servers[1].picking());
}

#[test]
fn a_taken_pick_answers_with_the_servers_name() {
    let answer = r#"{"id":"1100000000000000001","name":"Ana's server"}"#;
    assert_eq!(picked(answer).expect("an answer"), "Ana's server");
}

#[test]
fn an_answer_that_is_not_json_is_an_error_and_not_a_silent_pick() {
    assert!(picked("the forum goes in Ana's server").is_err());
}
