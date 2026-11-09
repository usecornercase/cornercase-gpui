use gpui::Keystroke;

use super::*;

const WRAP: Fit = Fit::Wrap { columns: 1 };

fn lines(texts: &[&str]) -> Vec<String> {
    texts.iter().map(|text| (*text).to_string()).collect()
}

fn caret(spot: usize, at: usize) -> Caret<usize> {
    Caret { spot, at }
}

fn chars(anchor: (usize, usize), head: (usize, usize)) -> Selection<usize> {
    Selection::Chars { anchor: caret(anchor.0, anchor.1), head: caret(head.0, head.1) }
}

fn whole(anchor: usize, head: usize) -> Selection<usize> {
    Selection::Lines { anchor, head }
}

fn moved(
    lines: &[String],
    fit: Fit,
    (spot, at, column): (usize, usize, Option<usize>),
    step: Step,
) -> (Caret<usize>, Option<usize>) {
    caret_step(lines, fit, (caret(spot, at), column), step)
}

fn keyed(lines: &[String], selection: Selection<usize>, step: Step) -> Selection<usize> {
    extended(lines, Fit::Cut, Keyed { selection, column: None }, step).selection
}

fn key(text: &str) -> Option<Step> {
    Step::of(&Keystroke::parse(text).expect("a keystroke"))
}

#[test]
fn shift_with_an_arrow_is_a_step_that_way() {
    assert_eq!(key("shift-left"), Some(Step::Left));
    assert_eq!(key("shift-right"), Some(Step::Right));
    assert_eq!(key("shift-up"), Some(Step::Up));
    assert_eq!(key("shift-down"), Some(Step::Down));
}

#[test]
fn an_arrow_without_shift_is_no_step() {
    assert_eq!(key("left"), None);
}

#[test]
fn shift_and_an_arrow_with_ctrl_alt_or_cmd_held_is_no_step() {
    for text in ["ctrl-shift-left", "alt-shift-right", "cmd-shift-up"] {
        assert_eq!(key(text), None);
    }
}

#[test]
fn shift_with_another_key_is_no_step() {
    assert_eq!(key("shift-home"), None);
    assert_eq!(key("shift-a"), None);
}

#[test]
fn right_moves_a_character_inside_a_line() {
    let text = lines(&["abc", "de"]);
    assert_eq!(moved(&text, Fit::Cut, (0, 1, None), Step::Right).0, caret(0, 2));
}

#[test]
fn right_at_the_end_of_a_line_goes_to_the_start_of_the_next() {
    let text = lines(&["abc", "de"]);
    assert_eq!(moved(&text, Fit::Cut, (0, 3, None), Step::Right).0, caret(1, 0));
}

#[test]
fn left_at_the_start_of_a_line_goes_to_the_end_of_the_one_before() {
    let text = lines(&["abc", "de"]);
    assert_eq!(moved(&text, Fit::Cut, (1, 0, None), Step::Left).0, caret(0, 3));
}

#[test]
fn a_character_of_several_bytes_is_one_step() {
    let text = lines(&["añ🦀b"]);
    let right = |at| moved(&text, Fit::Cut, (0, at, None), Step::Right).0.at;
    let left = |at| moved(&text, Fit::Cut, (0, at, None), Step::Left).0.at;
    assert_eq!((right(1), right(3)), (3, 7));
    assert_eq!((left(7), left(3)), (3, 1));
}

#[test]
fn a_letter_and_its_accent_are_one_step() {
    let text = lines(&["e\u{301}x"]);
    assert_eq!(moved(&text, Fit::Cut, (0, 0, None), Step::Right).0.at, 3);
    assert_eq!(moved(&text, Fit::Cut, (0, 3, None), Step::Left).0.at, 0);
}

#[test]
fn nothing_moves_left_of_the_first_line_or_right_of_the_last() {
    let text = lines(&["abc", "de"]);
    assert_eq!(moved(&text, Fit::Cut, (0, 0, None), Step::Left).0, caret(0, 0));
    assert_eq!(moved(&text, Fit::Cut, (1, 2, None), Step::Right).0, caret(1, 2));
}

#[test]
fn a_caret_inside_a_character_moves_from_where_it_starts() {
    let text = lines(&["añb"]);
    assert_eq!(moved(&text, Fit::Cut, (0, 2, None), Step::Right).0.at, 3);
}

#[test]
fn down_keeps_the_column() {
    let text = lines(&["abcdef", "ghijkl"]);
    assert_eq!(moved(&text, Fit::Cut, (0, 3, None), Step::Down), (caret(1, 3), Some(3)));
}

#[test]
fn a_shorter_line_holds_the_caret_at_its_end() {
    let text = lines(&["abcdef", "gh", "ijklmn"]);
    assert_eq!(moved(&text, Fit::Cut, (0, 4, None), Step::Down).0, caret(1, 2));
}

#[test]
fn the_column_comes_back_on_a_longer_line() {
    let text = lines(&["abcdef", "gh", "ijklmn"]);
    assert_eq!(moved(&text, Fit::Cut, (1, 2, Some(4)), Step::Down).0, caret(2, 4));
}

#[test]
fn the_column_counts_characters_not_bytes() {
    let text = lines(&["añb", "xyz"]);
    assert_eq!(moved(&text, Fit::Cut, (0, 3, None), Step::Down).0, caret(1, 2));
}

#[test]
fn up_on_the_first_line_goes_to_its_start_and_down_on_the_last_to_its_end() {
    let text = lines(&["abc", "de"]);
    assert_eq!(moved(&text, Fit::Cut, (0, 2, None), Step::Up).0, caret(0, 0));
    assert_eq!(moved(&text, Fit::Cut, (1, 1, None), Step::Down).0, caret(1, 2));
}

#[test]
fn left_or_right_forgets_the_column() {
    let text = lines(&["abc", "de"]);
    assert_eq!(moved(&text, Fit::Cut, (0, 1, Some(5)), Step::Right).1, None);
    assert_eq!(moved(&text, Fit::Cut, (0, 1, Some(5)), Step::Left).1, None);
}

#[test]
fn right_before_the_cut_stops_at_it() {
    let line = "a".repeat(99) + "e\u{301}" + &"a".repeat(50);
    let text = lines(&[line.as_str()]);
    assert_eq!(moved(&text, WRAP, (0, 99, None), Step::Right).0.at, 100);
}

#[test]
fn right_at_the_cut_goes_to_the_end_of_the_line_and_left_comes_back() {
    let text = lines(&["a".repeat(150).as_str()]);
    assert_eq!(moved(&text, WRAP, (0, 100, None), Step::Right).0.at, 150);
    assert_eq!(moved(&text, WRAP, (0, 150, None), Step::Left).0.at, 100);
}

#[test]
fn a_column_past_the_cut_lands_at_the_end_of_the_line() {
    let text = lines(&["x".repeat(130).as_str(), "a".repeat(150).as_str()]);
    assert_eq!(moved(&text, WRAP, (0, 130, None), Step::Down).0, caret(1, 150));
}

#[test]
fn a_cut_line_is_walked_into_its_hidden_end() {
    let text = lines(&["a".repeat(150).as_str()]);
    assert_eq!(moved(&text, Fit::Cut, (0, 100, None), Step::Right).0.at, 101);
}

#[test]
fn with_nothing_selected_a_step_starts_from_the_top_shown() {
    let text = lines(&["a", "b", "cd", "ef"]);
    let top = chars((2, 0), (2, 0));
    assert_eq!(keyed(&text, top, Step::Right), chars((2, 0), (2, 1)));
    let down = keyed(&text, top, Step::Down);
    assert_eq!(down, chars((2, 0), (3, 0)));
    assert_eq!(down.covered(), (2, 2));
}

#[test]
fn characters_grow_from_their_head_and_keep_their_anchor() {
    let text = lines(&["abc", "defgh", "ij"]);
    assert_eq!(keyed(&text, chars((1, 1), (1, 2)), Step::Right), chars((1, 1), (1, 3)));
    assert_eq!(keyed(&text, chars((2, 1), (1, 2)), Step::Left), chars((2, 1), (1, 1)));
}

#[test]
fn whole_lines_grow_and_shrink_by_lines_either_way() {
    let text = lines(&["a", "b", "c", "d", "e"]);
    let down = keyed(&text, whole(3, 3), Step::Down);
    assert_eq!(down, whole(3, 4));
    let up = keyed(&text, down, Step::Up);
    assert_eq!(up, whole(3, 3));
    assert_eq!(keyed(&text, up, Step::Up), whole(3, 2));
}

#[test]
fn whole_lines_stop_at_the_first_and_last_line() {
    let text = lines(&["a", "b", "c"]);
    assert_eq!(keyed(&text, whole(1, 0), Step::Up), whole(1, 0));
    assert_eq!(keyed(&text, whole(1, 2), Step::Down), whole(1, 2));
}

#[test]
fn left_or_right_turns_whole_lines_into_their_characters() {
    let text = lines(&["abc", "de", "fghi", "jk"]);
    assert_eq!(keyed(&text, whole(1, 2), Step::Right), chars((1, 0), (3, 0)));
    assert_eq!(keyed(&text, whole(2, 1), Step::Left), chars((2, 4), (0, 3)));
}

#[test]
fn a_head_back_on_its_anchor_selects_nothing_and_goes_on_from_there() {
    let text = lines(&["abcd", "efgh", "ij"]);
    let back = keyed(&text, chars((1, 2), (1, 3)), Step::Left);
    assert_eq!(back, chars((1, 2), (1, 2)));
    assert!(back.is_empty());
    assert_eq!(keyed(&text, back, Step::Left), chars((1, 2), (1, 1)));
}
