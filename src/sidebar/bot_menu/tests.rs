use super::*;
use crate::context_menu::read_entries;
use crate::panels::files::open::{Launch, Opener};

fn bot(state: &str) -> Bot {
    Bot { id: "prs".into(), name: "PRs".into(), state: state.into(), folder: "/work/prs".into(), ..Bot::default() }
}

fn vs_code() -> Vec<Found> {
    vec![(Opener::VsCode, Launch::Command("/bin/code".into()))]
}

fn read(bot: &Bot, from: MenuFrom, openers: &[Found], here: bool) -> Vec<(&'static str, String)> {
    read_entries(&bot_menu(bot, from, Offers { openers, here, routines: false, run_answers: false }))
}

fn read_with(bot: &Bot, from: MenuFrom, (routines, run_answers): (bool, bool)) -> Vec<(&'static str, String)> {
    let openers = vs_code();
    read_entries(&bot_menu(bot, from, Offers { openers: &openers, here: true, routines, run_answers }))
}

fn labels(rows: &[(&'static str, String)]) -> Vec<String> {
    rows.iter().map(|(_, label)| label.clone()).collect()
}

fn want(rows: &[(&'static str, &str)]) -> Vec<(&'static str, String)> {
    rows.iter().map(|(kind, label)| (*kind, label.to_string())).collect()
}

#[test]
fn an_idle_bots_row_menu_offers_every_entry_in_the_sidebars_order() {
    let rows = want(&[
        ("item", "New conversation…"),
        ("-", ""),
        ("item", "Details…"),
        ("item", "Rename bot"),
        ("item", "Edit bot…"),
        ("-", ""),
        ("item", "Open in VS Code"),
        ("item", "Copy path"),
        ("-", ""),
        ("danger", "Remove bot…"),
    ]);
    assert_eq!(read(&bot("idle"), MenuFrom::Row, &vs_code(), true), rows);
}

#[test]
fn while_a_bot_answers_its_conversation_and_removal_rest_and_say_why() {
    let rows = want(&[
        ("resting", "PRs is answering: stop it first"),
        ("resting", "New conversation…"),
        ("-", ""),
        ("item", "Details…"),
        ("item", "Rename bot"),
        ("item", "Edit bot…"),
        ("-", ""),
        ("item", "Open in VS Code"),
        ("item", "Copy path"),
        ("-", ""),
        ("resting", "Remove bot…"),
    ]);
    assert_eq!(read(&bot("thinking"), MenuFrom::Row, &vs_code(), true), rows);
}

#[test]
fn details_come_before_edit_in_both_menus_and_stay_while_the_bot_answers() {
    for from in [MenuFrom::Row, MenuFrom::Header] {
        for state in ["idle", "thinking"] {
            let rows = read(&bot(state), from, &vs_code(), true);
            let at = |label: &str| rows.iter().position(|(_, found)| found == label);
            assert!(at("Details…") < at("Edit bot…"), "{from:?} {state}: {rows:?}");
            assert!(rows.contains(&("item", "Details…".to_string())), "{from:?} {state}: {rows:?}");
        }
    }
}

#[test]
fn the_chat_headers_menu_leaves_rename_out() {
    let row = read(&bot("idle"), MenuFrom::Row, &vs_code(), true);
    let header = read(&bot("idle"), MenuFrom::Header, &vs_code(), true);
    assert!(!header.iter().any(|(_, label)| label == "Rename bot"));
    let without: Vec<_> = row.into_iter().filter(|(_, label)| label != "Rename bot").collect();
    assert_eq!(header, without);
}

#[test]
fn several_openers_share_the_open_in_submenu() {
    let two = [vs_code(), vec![(Opener::Zed, Launch::Command("/bin/zed".into()))]].concat();
    let rows = read(&bot("idle"), MenuFrom::Row, &two, true);
    assert!(rows.contains(&("submenu", "Open in…".to_string())), "{rows:?}");
}

#[test]
fn through_remote_a_bots_folder_offers_only_copy_path() {
    let rows = read(&bot("idle"), MenuFrom::Row, &vs_code(), false);
    assert!(rows.contains(&("item", "Copy path".to_string())));
    assert!(!rows.iter().any(|(_, label)| label.starts_with("Open in")));
}

#[test]
fn a_bot_without_a_folder_offers_neither_open_in_nor_copy_path() {
    let none = Bot { folder: String::new(), ..bot("idle") };
    let rows = read(&none, MenuFrom::Row, &vs_code(), true);
    let labels: Vec<&str> = rows.iter().map(|(_, label)| label.as_str()).collect();
    assert_eq!(labels, ["New conversation…", "", "Details…", "Rename bot", "Edit bot…", "", "Remove bot…"]);
}

#[test]
fn the_remove_question_says_what_goes_and_what_stays() {
    let (title, message, button) = remove_words("PRs", &[]);
    assert_eq!(title, "Remove the bot PRs?");
    assert_eq!(message, "Its chat, its memory and its folder are deleted. Agents it started keep running.");
    assert_eq!(button, "Remove");
}

#[test]
fn routines_is_listed_after_edit_only_when_offered_from_the_row_and_the_header() {
    for from in [MenuFrom::Row, MenuFrom::Header] {
        let offered = labels(&read_with(&bot("idle"), from, (true, false)));
        let edit = offered.iter().position(|label| label == "Edit bot…").expect("edit");
        assert_eq!(offered[edit + 1], "Routines…", "{from:?}");
        let not = labels(&read_with(&bot("idle"), from, (false, false)));
        assert!(!not.iter().any(|label| label == "Routines…"), "{from:?}");
    }
}

#[test]
fn while_a_run_answers_remove_rests_and_says_why_and_a_new_conversation_does_not() {
    let rows = read_with(&bot("idle"), MenuFrom::Row, (true, true));
    assert_eq!(rows[0], ("resting", "PRs is answering: stop it first".to_string()));
    assert_eq!(rows[1], ("item", "New conversation…".to_string()));
    assert_eq!(rows.last(), Some(&("resting", "Remove bot…".to_string())));
}

fn names(labels: &[&str]) -> Vec<String> {
    labels.iter().map(|label| (*label).to_string()).collect()
}

#[test]
fn the_remove_question_names_what_the_bot_scheduled() {
    let (_, message, _) = remove_words("PRs", &names(&["Review PRs", "Morning brief"]));
    assert_eq!(
        message,
        "Its chat, its memory, its folder and what it scheduled (Review PRs, Morning brief) are deleted. Agents it \
         started keep running."
    );
}

#[test]
fn the_remove_question_names_three_of_what_the_bot_scheduled_and_counts_the_rest() {
    let (_, message, _) = remove_words("PRs", &names(&["a", "b", "c", "d", "e"]));
    assert!(message.contains("what it scheduled (a, b, c and 2 more) are deleted"), "{message}");
}

fn on_vps(bot: Bot) -> Bot {
    Bot { home: crate::model::bots::Home::Machine("vps".into()), ..bot }
}

#[test]
fn a_machines_bot_offers_a_new_conversation_its_details_and_its_path_only() {
    let rows =
        want(&[("item", "New conversation…"), ("-", ""), ("item", "Details…"), ("-", ""), ("item", "Copy path")]);
    for from in [MenuFrom::Row, MenuFrom::Header] {
        assert_eq!(read_with(&on_vps(bot("idle")), from, (true, false)), rows, "{from:?}");
    }
}

#[test]
fn a_machines_bot_without_a_folder_offers_no_path() {
    let bot = on_vps(Bot { folder: String::new(), ..bot("idle") });
    assert_eq!(labels(&read_with(&bot, MenuFrom::Row, (true, false))), ["New conversation…", "", "Details…"]);
}

#[test]
fn while_a_machines_bot_answers_its_conversation_rests() {
    let rows = read_with(&on_vps(bot("thinking")), MenuFrom::Header, (false, false));
    assert_eq!(rows[..2], want(&[("resting", "PRs is answering: stop it first"), ("resting", "New conversation…")]));
    let run = read_with(&on_vps(bot("idle")), MenuFrom::Row, (false, true));
    assert_eq!(run[..2], want(&[("resting", "PRs is answering: stop it first"), ("item", "New conversation…")]));
}
