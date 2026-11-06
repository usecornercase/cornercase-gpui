use serde_json::Value;

use super::{Forum, Stage, of_app};
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
