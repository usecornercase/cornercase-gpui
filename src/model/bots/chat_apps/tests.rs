use serde_json::{Value, json};

use super::*;
use crate::cli::Version;
use crate::test_util::fixture;

fn app() -> Value {
    serde_json::from_str(fixture!("bots/app-chat-apps.json")).expect("fixture")
}

fn one(platform: &str, ready: bool, away: bool) -> ChatApp {
    ChatApp { platform: platform.to_string(), ready, away }
}

fn names(found: Option<Reach>) -> Vec<String> {
    found.map(|reach| reach.names).unwrap_or_default()
}

#[test]
fn the_platforms_are_read_from_the_app_line() {
    let app = app();
    assert_eq!(platforms(Some(&app)), vec![one("telegram", true, false), one("discord", false, false)]);
}

#[test]
fn an_app_line_without_away_rows_has_no_platform() {
    assert_eq!(platforms(None), Vec::new());
    assert_eq!(platforms(Some(&json!({}))), Vec::new());
    assert_eq!(platforms(Some(&json!({"away": null}))), Vec::new());
    assert_eq!(platforms(Some(&json!({"away": {"platform": "telegram"}}))), Vec::new());
}

#[test]
fn an_away_row_that_does_not_read_is_left_out_and_the_others_stay() {
    let app = json!({"away": [{"platform": 3}, {"platform": "discord", "ready": true}]});
    assert_eq!(platforms(Some(&app)), vec![one("discord", true, false)]);
}

#[test]
fn the_row_is_hidden_for_an_older_server() {
    let ready = [one("telegram", true, false)];
    let lacks = Some(Lacks::Server(Some(Version::new(0, 13, 106))));
    assert_eq!(row(lacks, &ready, Os::Mac), Row::Hidden);
}

#[test]
fn the_row_says_what_an_older_command_line_here_needs() {
    let lacks = Some(Lacks::ThisComputer(Some(Version::new(0, 13, 106))));
    let words = "Needs cornercase 0.13.107 or newer on this Mac (it has 0.13.106).";
    assert_eq!(row(lacks, &[], Os::Mac), Row::Lacks(words.to_string()));
}

#[test]
fn the_row_points_to_settings_with_no_chat_app_paired() {
    assert_eq!(row(None, &[], Os::Mac), Row::Unpaired);
    assert_eq!(row(None, &[one("telegram", false, true)], Os::Mac), Row::Unpaired);
}

#[test]
fn the_row_offers_the_choices_once_one_is_paired() {
    let platforms = [one("telegram", false, false), one("discord", true, false)];
    assert_eq!(row(None, &platforms, Os::Mac), Row::Choices);
}

#[test]
fn only_the_three_options_are_known() {
    for id in [AWAY, ALWAYS, OFF] {
        assert_eq!(known(id).as_deref(), Some(id));
    }
    assert_eq!(known(""), None);
    assert_eq!(known("sometimes"), None);
}

#[test]
fn a_choice_is_sent_only_while_the_row_offers_it() {
    assert_eq!(sent(&Row::Choices, Some(ALWAYS)).as_deref(), Some(ALWAYS));
    assert_eq!(sent(&Row::Choices, None), None);
    for row in [Row::Hidden, Row::Lacks("Needs".into()), Row::Unpaired] {
        assert_eq!(sent(&row, Some(ALWAYS)), None, "{row:?}");
    }
}

#[test]
fn each_choice_has_its_note_and_none_chosen_has_none() {
    let notes: Vec<_> = [AWAY, ALWAYS, OFF].into_iter().map(|id| note(Some(id))).collect();
    assert!(notes.iter().all(Option::is_some));
    assert_eq!(notes.iter().collect::<std::collections::HashSet<_>>().len(), 3);
    assert_eq!(note(None), None);
    assert_eq!(note(Some("sometimes")), None);
}

#[test]
fn always_reaches_a_ready_platform() {
    let found = reach(ALWAYS, &[one("telegram", true, false)]);
    assert_eq!(found, Some(Reach { names: vec!["Telegram".into()], always: true }));
}

#[test]
fn always_with_no_ready_platform_reaches_nothing() {
    assert_eq!(reach(ALWAYS, &[one("telegram", false, true)]), None);
    assert_eq!(reach(ALWAYS, &[]), None);
}

#[test]
fn while_away_reaches_a_ready_platform_that_is_away() {
    let found = reach(AWAY, &[one("discord", true, true)]);
    assert_eq!(found, Some(Reach { names: vec!["Discord".into()], always: false }));
}

#[test]
fn while_away_reaches_nothing_while_you_are_not_away() {
    assert_eq!(reach(AWAY, &[one("telegram", true, false)]), None);
}

#[test]
fn while_away_names_only_the_platforms_that_are_away() {
    let found = reach(AWAY, &[one("telegram", true, false), one("discord", true, true)]);
    assert_eq!(names(found), vec!["Discord".to_string()]);
}

#[test]
fn off_reaches_nothing() {
    assert_eq!(reach(OFF, &[one("telegram", true, true)]), None);
}

#[test]
fn an_unknown_or_empty_option_reaches_nothing() {
    assert_eq!(reach("", &[one("telegram", true, true)]), None);
    assert_eq!(reach("sometimes", &[one("telegram", true, true)]), None);
}

#[test]
fn a_platform_away_but_not_ready_does_not_count_for_another_ready_one() {
    assert_eq!(reach(AWAY, &[one("telegram", true, false), one("discord", false, true)]), None);
}

#[test]
fn an_unknown_platform_is_named_as_it_comes() {
    assert_eq!(names(reach(ALWAYS, &[one("slack", true, false)])), vec!["slack".to_string()]);
}

#[test]
fn the_tip_names_where_the_answers_go() {
    let one_name = Reach { names: vec!["Telegram".into()], always: true };
    assert_eq!(reach_tip(&one_name), "Its answers also go to Telegram.");
    let two = Reach { names: vec!["Telegram".into(), "Discord".into()], always: true };
    assert_eq!(reach_tip(&two), "Its answers also go to Telegram and Discord.");
    let away = Reach { names: vec!["Telegram".into()], always: false };
    assert_eq!(reach_tip(&away), "Its answers also go to Telegram while you are away.");
}
