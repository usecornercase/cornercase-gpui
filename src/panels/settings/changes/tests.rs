use super::*;
use crate::test_util::fixture;

fn phone_paired() -> data::Described {
    data::parse(fixture!("settings/phone-app-paired.json")).expect("the paired answer parses")
}

fn key_of_account(d: &data::Described, account: &str, nth: usize) -> String {
    let mut named = d.connections.iter().filter(|c| c.account.as_deref() == Some(account));
    named.nth(nth).expect("that app").key.clone()
}

fn app(account: Option<&str>) -> Connection {
    Connection {
        key: "app.0011223344556677".into(),
        kind: ConnectionKind::App,
        label: "paired app".into(),
        note: "paired 2026-11-14 · enter forgets it".into(),
        state: data::State::Connected,
        account: account.map(str::to_string),
        removable: true,
        ..Connection::default()
    }
}

#[test]
fn the_question_for_a_paired_app_names_it_and_says_it_is_disconnected_at_once() {
    let d = phone_paired();
    let key = key_of_account(&d, "phone app", 0);
    assert_eq!(
        removal(&d.connections, &key),
        Some(Ask {
            title: "Forget “phone app”?".into(),
            message: "Paired 2026-11-14. It is disconnected at once and has to be paired again to reach this computer."
                .into(),
            button: "Forget",
        })
    );
}

#[test]
fn the_question_for_one_of_two_alike_apps_says_which() {
    let d = phone_paired();
    let key = key_of_account(&d, "cornercase-mobile/0.1.0 (Ana's iPhone)", 1);
    let ask = removal(&d.connections, &key).expect("a question");
    assert_eq!(ask.title, "Forget “cornercase-mobile/0.1.0 (Ana's iPhone)” (2 of 2)?");
}

#[test]
fn the_question_for_an_app_with_a_name_of_its_own_quotes_it() {
    let ask = removal(&[app(Some("Ana's iPhone"))], "app.0011223344556677").expect("a question");
    assert_eq!(ask.title, "Forget “Ana's iPhone”?");
}

#[test]
fn the_question_for_an_app_with_no_name_at_all_says_this_phone_app() {
    let ask = removal(&[app(None)], "app.0011223344556677").expect("a question");
    assert_eq!(ask.title, "Forget this phone app?");
}

#[test]
fn the_questions_of_browsers_chats_and_tokens_are_as_they_were() {
    let of = |kind, label: &str| Connection { key: "k".into(), kind, label: label.into(), ..Connection::default() };
    let ask = |kind, label| removal(&[of(kind, label)], "k").expect("a question");
    assert_eq!(
        ask(ConnectionKind::Browsers, "paired browsers"),
        Ask {
            title: "Forget every paired browser?".into(),
            message: "Each one has to be paired again to reach this computer.".into(),
            button: "Forget",
        }
    );
    assert_eq!(
        ask(ConnectionKind::Chat, "Telegram phone"),
        Ask {
            title: "Forget the Telegram phone?".into(),
            message: "Agents stop messaging it, and it has to be paired again.".into(),
            button: "Forget",
        }
    );
    assert_eq!(
        ask(ConnectionKind::Tracker, "Linear API key"),
        Ask {
            title: "Remove the Linear API key?".into(),
            message: "cornercase forgets it, and it has to be pasted again to be used.".into(),
            button: "Remove",
        }
    );
}

#[test]
fn a_key_the_sheet_does_not_list_asks_nothing() {
    assert_eq!(removal(&phone_paired().connections, "app.none"), None);
}

#[test]
fn the_question_for_an_app_with_no_note_keeps_only_what_happens_next() {
    for note in ["", "enter forgets it"] {
        let mut c = app(Some("Ana"));
        c.note = note.into();
        let ask = removal(&[c], "app.0011223344556677").expect("a question");
        assert_eq!(ask.message, "It is disconnected at once and has to be paired again to reach this computer.");
    }
}

#[test]
fn a_field_typed_in_while_its_change_ran_keeps_what_was_typed() {
    assert_eq!(field_shows("cc-{key}", "cc-{key}z", true), FieldShows::Typed);
    assert_eq!(field_shows("cc-{key}", "cc-{key}z", false), FieldShows::Typed);
}

#[test]
fn a_field_left_as_it_was_sent_takes_its_answer() {
    assert_eq!(field_shows("a", "a", true), FieldShows::Answer);
}

#[test]
fn a_field_left_as_it_was_sent_shows_what_the_sheet_holds_when_its_answer_is_older() {
    assert_eq!(field_shows("a", "a", false), FieldShows::Held);
}
