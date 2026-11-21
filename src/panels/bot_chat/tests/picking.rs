use std::rc::Rc;

use super::super::entries::{Block, Entry};
use super::super::picking::{ERROR_PART, Spot, parts, selected_text, text_at};
use crate::ui::markdown::{self, Block as Shown};
use crate::ui::selection::{Caret, Selection};

const ANSWER: &str = "Start of the paragraph here.\n\n- one\n- two\n\n```rust\nfn a() {\n\tb();\n}\n```";

fn parsed(_: u64, text: &str) -> Rc<Vec<Shown>> {
    Rc::new(markdown::parse(text))
}

fn spot(entry: u64, part: u32) -> Spot {
    Spot { entry, part }
}

fn you(id: u64, text: &str) -> Entry {
    Entry::You { id, text: text.into(), at: 0, held: false, dropped: false }
}

fn run() -> Entry {
    Entry::Run(Block {
        run: 7,
        head: 3,
        tail: 10,
        lines: 4,
        title: "Review PRs".into(),
        steps: vec!["read".into()],
        answer: Some((9, "Run *answer*.".into(), 0)),
        error: Some("It broke.".into()),
        end: None,
        others: vec!["other".into()],
    })
}

fn chat() -> Vec<Entry> {
    vec![you(1, "Hi there"), Entry::Answer { id: 2, text: ANSWER.into(), at: 0 }, run(), you(5, "Thanks")]
}

fn chars((from, at): (Spot, usize), (to, end): (Spot, usize)) -> Selection<Spot> {
    Selection::Chars { anchor: Caret { spot: from, at }, head: Caret { spot: to, at: end } }
}

fn copied(selection: Selection<Spot>) -> String {
    selected_text(chat().iter(), selection, &parsed)
}

fn spots(entry: &Entry) -> Vec<Spot> {
    parts(entry, &parsed).into_iter().map(|(spot, _)| spot).collect()
}

#[test]
fn each_kind_of_entry_has_the_parts_it_draws_as_text() {
    assert_eq!(parts(&you(1, "Hi"), &parsed), [(spot(1, 0), "Hi".to_string())]);
    assert_eq!(spots(&chat()[1]), [spot(2, 0), spot(2, 1), spot(2, 2), spot(2, 3)]);
    assert_eq!(
        parts(&run(), &parsed),
        [(spot(3, 0), "Run answer.".to_string()), (spot(3, ERROR_PART), "It broke.".to_string())]
    );
    assert_eq!(parts(&Entry::Failed { id: 4, text: "No".into() }, &parsed), [(spot(4, 0), "No".to_string())]);
    let none = [
        Entry::Steps { first: 1, last: 2, texts: vec!["a".into()] },
        Entry::Notice { id: 1, text: "stopped".into(), ask: None },
        Entry::Other { id: 1, text: "odd".into() },
    ];
    assert!(none.iter().all(|entry| spots(entry).is_empty()));
}

#[test]
fn spots_sort_in_the_order_the_chat_draws_them() {
    let all: Vec<Spot> = chat().iter().flat_map(spots).collect();
    let mut sorted = all.clone();
    sorted.sort();
    assert_eq!(all, sorted);
    assert!(spot(3, ERROR_PART) < spot(5, 0));
}

#[test]
fn a_copy_runs_from_a_paragraph_over_a_list_into_a_code_block_as_drawn() {
    let selection = chars((spot(2, 0), 9), (spot(2, 3), 12));
    assert_eq!(copied(selection), "the paragraph here.\none\ntwo\nfn a() {\n   ");
}

#[test]
fn a_copy_runs_from_a_bubble_into_the_answer_under_it() {
    assert_eq!(copied(chars((spot(1, 0), 3), (spot(2, 0), 5))), "there\nStart");
}

#[test]
fn a_copy_backwards_reads_the_same() {
    assert_eq!(copied(chars((spot(2, 0), 5), (spot(1, 0), 3))), "there\nStart");
}

#[test]
fn a_selection_whose_first_part_was_dropped_copies_what_is_left() {
    assert_eq!(copied(chars((spot(0, 0), 4), (spot(1, 0), 2))), "Hi");
}

#[test]
fn a_whole_part_copies_its_text() {
    assert_eq!(copied(Selection::line(spot(3, ERROR_PART))), "It broke.");
}

#[test]
fn the_text_at_a_spot_is_that_parts_drawn_text() {
    assert_eq!(text_at(chat().iter(), spot(2, 1), &parsed).as_deref(), Some("one"));
    assert_eq!(text_at(chat().iter(), spot(3, 0), &parsed).as_deref(), Some("Run answer."));
    assert_eq!(text_at(chat().iter(), spot(4, 0), &parsed), None);
    assert_eq!(text_at(chat().iter(), spot(2, 9), &parsed), None);
}
