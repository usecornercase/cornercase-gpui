use super::pairing_note;
use crate::panels::settings::data::{Connection, ConnectionKind, State};

fn chat(state: State, note: &str) -> Connection {
    Connection { kind: ConnectionKind::Chat, state, note: note.to_string(), ..Connection::default() }
}

#[test]
fn an_unpaired_chat_says_where_to_pair_it_and_what_pairing_here_waits_on() {
    let note = pairing_note(&chat(State::Missing, "enter shows a code to scan with your phone")).expect("a note");
    assert_eq!(
        note,
        "Pair it from cornercase's own window, Settings › Remote: it shows a code to scan with your phone. \
         Pairing here needs usecornercase/cornercase-terminal#670."
    );
}

#[test]
fn a_paired_chat_has_nothing_to_wait_on() {
    assert_eq!(pairing_note(&chat(State::Connected, "enter forgets it")), None);
}

#[test]
fn paired_browsers_say_what_pairing_another_waits_on() {
    let browsers = Connection { kind: ConnectionKind::Browsers, count: Some(2), ..Connection::default() };
    assert!(pairing_note(&browsers).expect("a note").ends_with("needs usecornercase/cornercase-terminal#669."));
}

#[test]
fn a_bot_is_set_here_so_it_says_nothing_about_pairing() {
    let bot = Connection { kind: ConnectionKind::Bot, ..Connection::default() };
    assert_eq!(pairing_note(&bot), None);
}
