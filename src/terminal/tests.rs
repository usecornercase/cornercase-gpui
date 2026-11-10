use gpui::CursorStyle;

use super::pane_menu::move_pane_args;
use super::selection::{cleaned, kept, read_args, word_at};
use super::{cycled, divider_cursor, note_text, split_args};
use crate::app::{Conn, Msg};
use crate::desktop::Board;
use crate::platform::paths::Style;
use crate::render::moving::{Aim, Place};

#[test]
fn a_divider_shows_the_way_it_moves() {
    assert_eq!(divider_cursor(true), CursorStyle::ResizeRow);
    assert_eq!(divider_cursor(false), CursorStyle::ResizeColumn);
}

#[test]
fn a_dropped_pane_asks_cornercase_to_move_it_beside_its_target() {
    let args = move_pane_args(4, Aim { target: 7, place: Place::Below });
    assert_eq!(args, ["move-pane", "--pane", "4", "--target", "7", "--place", "below"]);
}

#[test]
fn a_word_is_a_run_of_one_class() {
    let text = "ls ~/src/web-shop && echo";
    let line: Vec<&str> = text.char_indices().map(|(i, c)| &text[i..i + c.len_utf8()]).collect();
    assert_eq!(word_at(&line, 5, Style::Unix), (3, 16));
    assert_eq!(word_at(&line, 0, Style::Unix), (0, 1));
    assert_eq!(word_at(&line, 18, Style::Unix), (18, 19));
    assert_eq!(word_at(&["a", "中", "", "b", " "], 2, Style::Unix), (0, 3));
    assert_eq!(word_at(&["a"], 4, Style::Unix), (4, 4));
}

#[test]
fn a_backslash_joins_a_word_only_on_a_windows_server() {
    let text = r"type C:\Users\me\x.rs";
    let line: Vec<&str> = text.char_indices().map(|(i, c)| &text[i..i + c.len_utf8()]).collect();
    assert_eq!(word_at(&line, 12, Style::Windows), (5, 20));
    assert_eq!(word_at(&line, 12, Style::Unix), (8, 12));
}

#[test]
fn cycling_wraps_around_both_ways() {
    let ids = [4, 7, 9];
    assert_eq!(cycled(&ids, Some(9), 1), Some(4));
    assert_eq!(cycled(&ids, Some(4), -1), Some(9));
    assert_eq!(cycled(&ids, Some(7), 1), Some(9));
    assert_eq!(cycled(&ids, None, 1), Some(7));
    assert_eq!(cycled(&[], Some(1), 1), None);
}

#[test]
fn copying_asks_cornercase_to_copy_while_the_primary_selection_only_reads_the_text() {
    assert_eq!(read_args(4, Board::Clipboard, false), ["selection", "--pane", "4", "--copy", "--json"]);
    assert_eq!(read_args(4, Board::Primary, false), ["selection", "--pane", "4", "--json"]);
}

#[test]
fn a_read_selection_goes_to_its_board() {
    let read = Ok(r#"{"text":"ls -la"}"#.to_string());
    assert!(matches!(kept(Board::Primary, read, false), Some(Msg::Copied(Board::Primary, text)) if text == "ls -la"));
    assert!(kept(Board::Primary, Ok(r#"{"text":""}"#.to_string()), false).is_none());
}

#[test]
fn a_pane_with_nothing_selected_leaves_the_board_alone() {
    assert!(kept(Board::Primary, Ok(r#"{"pane":4}"#.to_string()), false).is_none());
    assert!(kept(Board::Primary, Ok(r#"{"pane":4,"text":null}"#.to_string()), false).is_none());
}

#[test]
fn a_selection_that_cannot_be_read_says_why() {
    let read = Err(anyhow::anyhow!("no such pane"));
    assert!(matches!(kept(Board::Primary, read, false), Some(Msg::Error(e)) if e == "no such pane"));
    for answer in ["Selected: ls -la", r#"{"text":7}"#] {
        let said = kept(Board::Clipboard, Ok(answer.to_string()), false);
        assert!(matches!(said, Some(Msg::Error(e)) if e.starts_with("could not read the selection")), "{answer}");
    }
}

#[test]
fn a_selection_on_a_machine_is_read_and_never_copied_there() {
    assert_eq!(read_args(4, Board::Clipboard, true), ["selection", "--pane", "4", "--json"]);
}

#[test]
fn a_selection_from_a_machine_reaches_the_clipboard_cleaned() {
    let read = Ok(serde_json::json!({ "text": "ls\u{1b}[0m\u{202e} -la\n\tnext" }).to_string());
    let said = kept(Board::Clipboard, read, true);
    assert!(matches!(said, Some(Msg::Copied(Board::Clipboard, text)) if text == "ls[0m -la\n\tnext"));
}

#[test]
fn a_selection_from_a_machine_with_nothing_left_once_cleaned_copies_nothing() {
    let read = Ok(serde_json::json!({ "text": "\u{1b}\u{200b}" }).to_string());
    assert!(kept(Board::Clipboard, read, true).is_none());
}

#[test]
fn what_is_cleaned_keeps_lines_tabs_and_every_visible_character() {
    assert_eq!(cleaned("a\tb\nñ中 🦀"), "a\tb\nñ中 🦀");
}

#[test]
fn a_split_names_its_pane_and_asks_to_focus_the_new_one() {
    assert_eq!(split_args(5, false), ["split", "--pane", "5", "--focus"]);
    assert_eq!(split_args(5, true), ["split", "--pane", "5", "--focus", "--down"]);
}

fn lost(why: &str) -> Conn {
    Conn::Lost(why.to_string())
}

#[test]
fn a_connection_that_never_came_up_is_not_called_lost() {
    let title = note_text(&lost("x"), None, Some("vps"), false).0;
    assert_eq!(title, "Could not connect to cornercase on vps");
}

#[test]
fn a_connection_that_dropped_is_lost() {
    assert_eq!(note_text(&lost("x"), None, Some("vps"), true).0, "Lost cornercase on vps");
    assert_eq!(note_text(&lost("x"), None, None, true).0, "Lost cornercase");
}

#[test]
fn the_reason_loses_cornercases_prefix() {
    let conn = lost("cornercase start-server: Error: `vps` runs cornercase 0.13.18 and this machine 0.13.21");
    let detail = note_text(&conn, None, Some("vps"), false).1.expect("a reason");
    assert!(detail.starts_with("`vps` runs"), "{detail}");
}
