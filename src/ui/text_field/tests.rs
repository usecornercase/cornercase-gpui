use super::{Escaped, escaped, selected_rows, word_end, word_start};

#[test]
fn a_selection_inside_one_row_paints_one_span() {
    assert_eq!(selected_rows(&[0, 10, 20], 25, &(12..15)), vec![(1, 12..15)]);
}

#[test]
fn a_selection_across_wraps_paints_a_span_per_row() {
    assert_eq!(selected_rows(&[0, 10, 20], 25, &(5..22)), vec![(0, 5..10), (1, 10..20), (2, 20..22)]);
}

#[test]
fn a_selection_ending_at_a_wrap_leaves_the_next_row_alone() {
    assert_eq!(selected_rows(&[0, 10], 18, &(3..10)), vec![(0, 3..10)]);
}

#[test]
fn an_unwrapped_line_is_one_row() {
    assert_eq!(selected_rows(&[0], 8, &(0..8)), vec![(0, 0..8)]);
}

#[test]
fn escape_reverts_an_edited_field_then_passes_on() {
    assert_eq!(escaped(false, "typed", Some("saved")), Escaped::Revert);
    assert_eq!(escaped(false, "saved", Some("saved")), Escaped::Pass);
}

#[test]
fn escape_clears_a_search_then_passes_on() {
    assert_eq!(escaped(true, "query", None), Escaped::Clear);
    assert_eq!(escaped(true, "", None), Escaped::Pass);
}

#[test]
fn escape_cancels_a_plain_field() {
    assert_eq!(escaped(false, "name", None), Escaped::Cancel);
}

#[test]
fn a_word_starts_after_the_blank_before_it() {
    assert_eq!(word_start("cargo test -q", 13), 11);
    assert_eq!(word_start("cargo test  ", 12), 6);
    assert_eq!(word_start("word", 4), 0);
    assert_eq!(word_start("año\u{3000}más", "año\u{3000}más".len()), "año\u{3000}".len());
}

#[test]
fn a_word_ends_at_the_blank_after_it() {
    assert_eq!(word_end("cargo test", 0), 5);
    assert_eq!(word_end("cargo test", 5), 10);
    assert_eq!(word_end("cargo  test -q", 5), 11);
    assert_eq!(word_end("word", 4), 4);
}
