use super::*;
use crate::context_menu::read_entries;

fn bot(state: &str) -> Bot {
    Bot { id: "prs".into(), name: "PRs".into(), state: state.into(), ..Bot::default() }
}

#[test]
fn an_idle_bots_menu_offers_a_new_conversation_and_its_removal() {
    let entries = read_entries(&bot_menu(&bot("idle")));
    let want = [("item", "New conversation…"), ("-", ""), ("danger", "Remove bot…")];
    assert_eq!(entries, want.map(|(kind, label)| (kind, label.to_string())));
}

#[test]
fn while_a_bot_answers_its_menu_rests_and_says_why() {
    let entries = read_entries(&bot_menu(&bot("thinking")));
    let want = [
        ("resting", "PRs is answering: stop it first"),
        ("resting", "New conversation…"),
        ("-", ""),
        ("resting", "Remove bot…"),
    ];
    assert_eq!(entries, want.map(|(kind, label)| (kind, label.to_string())));
}

#[test]
fn the_remove_question_says_what_goes_and_what_stays() {
    let (title, message, button) = remove_words("PRs");
    assert_eq!(title, "Remove the bot PRs?");
    assert_eq!(message, "Its chat, its memory and its folder are deleted. Agents it started keep running.");
    assert_eq!(button, "Remove");
}
