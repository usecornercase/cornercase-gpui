use std::collections::HashSet;

use super::words::{folded_label, time_label, turn_label};
use super::{Scroll, row_keys, scroll_distance, scroll_of};
use crate::model::bots::entries::{Entry, Turn};

const NOON: i64 = 1_794_744_000;

#[test]
fn a_line_of_today_shows_its_time_of_day() {
    let label = time_label(NOON, NOON + 60);
    assert_eq!(label.len(), 5);
    assert_eq!(label.as_bytes()[2], b':');
}

#[test]
fn an_older_line_shows_its_day_and_time() {
    let label = time_label(NOON, NOON + 3 * 86_400);
    assert!(label.len() > 5 && label.ends_with(&time_label(NOON, NOON + 1)), "{label}");
}

#[test]
fn a_folded_run_reads_plus_n_steps() {
    assert_eq!(folded_label(4), "+4 steps");
}

#[test]
fn one_hidden_step_reads_plus_1_step() {
    assert_eq!(folded_label(1), "+1 step");
}

#[test]
fn the_turn_row_reads_thinking_its_time_and_its_steps() {
    assert_eq!(turn_label(Turn { since: 100, steps: 3 }, 142), "Thinking · 42s · 3 steps");
    assert_eq!(turn_label(Turn { since: 100, steps: 1 }, 160), "Thinking · 1m · 1 step");
}

#[test]
fn the_turn_row_leaves_out_a_time_of_now() {
    assert_eq!(turn_label(Turn { since: 100, steps: 0 }, 102), "Thinking");
}

#[test]
fn the_arrows_and_the_pages_scroll_the_chat() {
    assert_eq!(scroll_of("up"), Some(Scroll::Line(-1.)));
    assert_eq!(scroll_of("down"), Some(Scroll::Line(1.)));
    assert_eq!(scroll_of("pageup"), Some(Scroll::Page(-1.)));
    assert_eq!(scroll_of("pagedown"), Some(Scroll::Page(1.)));
    assert!(scroll_distance(Scroll::Page(-1.), 600.) < -scroll_distance(Scroll::Line(1.), 600.));
}

#[test]
fn other_keys_scroll_nothing() {
    for key in ["a", "enter", "escape", "left", "space"] {
        assert_eq!(scroll_of(key), None, "{key}");
    }
}

#[test]
fn an_answer_keeps_its_raw_markdown_for_its_copy() {
    let lines = vec![crate::model::bots::Line {
        id: 1,
        at: 1,
        kind: crate::model::bots::Kind::Bot,
        text: "**bold** and `code`".into(),
        held: false,
    }];
    let entries = crate::model::bots::entries::entries(&lines);
    assert!(matches!(&entries[0], Entry::Answer { text, .. } if text == "**bold** and `code`"));
}

#[test]
fn a_note_of_earlier_lines_comes_first_in_the_keys() {
    let entries = [Entry::Notice { id: 5, text: "n".into() }];
    let with = row_keys(true, &entries, &HashSet::new());
    let without = row_keys(false, &entries, &HashSet::new());
    assert_eq!((with.len(), without.len()), (2, 1));
}
