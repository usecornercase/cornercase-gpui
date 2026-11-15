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
    read_entries(&bot_menu(bot, from, (openers, here)))
}

fn want(rows: &[(&'static str, &str)]) -> Vec<(&'static str, String)> {
    rows.iter().map(|(kind, label)| (*kind, label.to_string())).collect()
}

#[test]
fn an_idle_bots_row_menu_offers_every_entry_in_the_sidebars_order() {
    let rows = want(&[
        ("item", "New conversation…"),
        ("-", ""),
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
    assert_eq!(labels, ["New conversation…", "", "Rename bot", "Edit bot…", "", "Remove bot…"]);
}

#[test]
fn the_remove_question_says_what_goes_and_what_stays() {
    let (title, message, button) = remove_words("PRs");
    assert_eq!(title, "Remove the bot PRs?");
    assert_eq!(message, "Its chat, its memory and its folder are deleted. Agents it started keep running.");
    assert_eq!(button, "Remove");
}
