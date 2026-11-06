use gpui::{Hsla, px};

use super::*;

fn lit() -> HighlightStyle {
    HighlightStyle { background_color: Some(Hsla::default()), ..HighlightStyle::default() }
}

#[test]
fn a_line_within_the_cap_is_shown_whole() {
    let line = capped("short", vec![(0..5, lit())], 10);
    assert_eq!((line.shown.as_str(), line.kept), ("short", None));
    assert_eq!(line.styles, vec![(0..5, lit())]);
}

#[test]
fn styles_left_from_an_older_text_are_dropped_rather_than_drawn_wrong() {
    let plain = HighlightStyle::default();
    let line = capped("ñb", vec![(0..1, plain), (0..2, lit()), (2..9, plain)], 10);
    assert_eq!(line.styles, vec![(0..2, lit())]);
}

#[test]
fn an_empty_line_shows_a_stand_in_so_its_row_has_a_height() {
    assert_eq!(capped("", Vec::new(), 10).shown, " ");
}

#[test]
fn a_line_past_the_cap_ends_in_an_ellipsis_and_keeps_its_start() {
    let line = capped("abcdefgh", Vec::new(), 4);
    assert_eq!((line.shown.as_str(), line.kept), ("abcd…", Some(4)));
}

#[test]
fn the_cap_never_cuts_inside_a_character() {
    let line = capped("aññb", Vec::new(), 2);
    assert_eq!((line.shown.as_str(), line.kept), ("a…", Some(1)));
}

#[test]
fn styles_are_cut_to_what_stays_shown() {
    let plain = HighlightStyle::default();
    let line = capped("abcdefgh", vec![(0..2, plain), (2..6, plain), (6..8, plain)], 4);
    assert_eq!(line.styles[..2], [(0..2, plain), (2..4, plain)]);
}

#[test]
fn the_ellipsis_takes_the_tint_of_what_is_lit_behind_it() {
    let line = capped("abcdefgh", vec![(6..8, lit())], 4);
    assert_eq!(line.styles, vec![(4..7, lit())]);
}

#[test]
fn the_ellipsis_is_plain_with_nothing_lit_behind_it() {
    let line = capped("abcdefgh", vec![(0..2, lit())], 4);
    assert_eq!(line.styles[1], (4..7, HighlightStyle::default()));
}

#[test]
fn a_caret_past_the_cut_stands_for_the_rest_of_the_line() {
    assert_eq!(past_the_cut(7, Some(4), 900), 900);
}

#[test]
fn a_caret_before_the_cut_stays_where_it_is() {
    assert_eq!(past_the_cut(4, Some(4), 900), 4);
    assert_eq!(past_the_cut(7, None, 900), 7);
}

#[test]
fn wrapping_allows_a_hundred_rows_of_the_columns_that_fit() {
    assert_eq!(Fit::of(true, px(730.)).cap(), Some(10_000));
}

#[test]
fn before_the_first_frame_the_width_is_taken_as_wide() {
    assert_eq!(Fit::of(true, px(0.)), Fit::Wrap { columns: UNKNOWN_COLUMNS });
}

#[test]
fn cutting_has_no_cap_of_its_own() {
    assert_eq!(Fit::of(false, px(730.)), Fit::Cut);
    assert_eq!(Fit::Cut.cap(), None);
}

#[test]
fn lights_are_worked_out_a_little_past_where_a_line_ends_on_screen() {
    assert_eq!(Fit::Cut.lit_bytes(), LIT_PAST_THE_CUT);
    assert_eq!(Fit::Wrap { columns: 10 }.lit_bytes(), 1000 + LIT_PAST_THE_CUT);
}

#[test]
fn a_cut_line_takes_one_row_whatever_its_length() {
    assert_eq!(Fit::Cut.rows_of(9000), 1);
    assert_eq!(Fit::Cut.row_at(9000), 0);
}

#[test]
fn a_wrapped_line_takes_the_rows_its_length_needs_up_to_the_cap() {
    let fit = Fit::Wrap { columns: 10 };
    assert_eq!((fit.rows_of(0), fit.rows_of(10), fit.rows_of(11)), (1, 1, 2));
    assert_eq!(fit.rows_of(1_000_000), WRAP_ROWS);
}

#[test]
fn a_byte_of_a_wrapped_line_falls_on_the_row_its_offset_says() {
    let fit = Fit::Wrap { columns: 10 };
    assert_eq!((fit.row_at(9), fit.row_at(10)), (0, 1));
    assert_eq!(fit.row_at(1_000_000), WRAP_ROWS - 1);
}
