use gpui::px;

use super::{
    Rows, Span, clamp_scroll, line_breaks, one_line, previous_grapheme, rows_shown, scroll_to_show,
    scroll_to_show_with_margins, sideways, thumb,
};

fn wrapped() -> Rows {
    Rows::new([(25, vec![0, 10, 20])])
}

fn two_lines() -> Rows {
    Rows::new([(5, vec![0]), (0, vec![0]), (12, vec![0, 6])])
}

fn span(row: usize, range: std::ops::Range<usize>, line_break: bool) -> Span {
    Span { row, range, line_break }
}

#[test]
fn a_selection_inside_one_row_paints_one_span() {
    assert_eq!(wrapped().selected(&(12..15)), vec![span(1, 12..15, false)]);
}

#[test]
fn a_selection_across_wraps_paints_a_span_per_row() {
    assert_eq!(
        wrapped().selected(&(5..22)),
        vec![span(0, 5..10, false), span(1, 10..20, false), span(2, 20..22, false)]
    );
}

#[test]
fn a_selection_ending_at_a_wrap_leaves_the_next_row_alone() {
    assert_eq!(Rows::new([(18, vec![0, 10])]).selected(&(3..10)), vec![span(0, 3..10, false)]);
}

#[test]
fn an_unwrapped_line_is_one_row() {
    assert_eq!(Rows::new([(8, vec![0])]).selected(&(0..8)), vec![span(0, 0..8, false)]);
}

#[test]
fn a_selection_over_a_line_break_marks_it_on_its_row() {
    assert_eq!(two_lines().selected(&(3..6)), vec![span(0, 3..5, true)]);
}

#[test]
fn a_selected_empty_line_still_shows_its_break() {
    assert_eq!(two_lines().selected(&(5..7)), vec![span(0, 5..5, true), span(1, 6..6, true)]);
}

#[test]
fn a_selection_up_to_a_line_end_leaves_its_break_alone() {
    assert_eq!(two_lines().selected(&(3..5)), vec![span(0, 3..5, false)]);
}

#[test]
fn offsets_count_the_line_breaks_between_lines() {
    let rows = two_lines();
    let starts: Vec<usize> = (0..rows.len()).filter_map(|row| rows.get(row)).map(|row| row.start).collect();
    assert_eq!(starts, [0, 6, 7, 13]);
}

#[test]
fn a_line_end_belongs_to_the_row_it_ends() {
    assert_eq!(two_lines().row_of(5), 0);
}

#[test]
fn an_empty_line_is_a_row_of_its_own() {
    assert_eq!(two_lines().row_of(6), 1);
}

#[test]
fn an_offset_at_a_wrap_belongs_to_the_next_row() {
    assert_eq!(wrapped().row_of(10), 1);
    assert_eq!(wrapped().row_of(9), 0);
}

#[test]
fn the_end_of_the_text_is_on_the_last_row() {
    assert_eq!(wrapped().row_of(25), 2);
}

#[test]
fn a_wrapped_row_ends_before_the_grapheme_that_wraps() {
    let text = "first row second";
    let rows = Rows::new([(text.len(), vec![0, 10])]);
    assert_eq!(rows.get(0).expect("row").last_caret(text), 9);
}

#[test]
fn a_row_that_ends_its_line_ends_at_the_line_end() {
    let text = "first row second";
    let rows = Rows::new([(text.len(), vec![0, 10])]);
    assert_eq!(rows.get(1).expect("row").last_caret(text), text.len());
}

#[test]
fn a_row_that_starts_inside_a_cluster_never_ends_before_it_starts() {
    let text = "ae\u{301}b";
    let rows = Rows::new([(text.len(), vec![0, 2, 4])]);
    assert_eq!(rows.get(1).expect("row").last_caret(text), 2);
}

#[test]
fn the_previous_grapheme_keeps_clusters_whole() {
    assert_eq!(previous_grapheme("ae\u{301}x", 4), 1);
}

#[test]
fn line_breaks_of_any_kind_become_one_line_feed() {
    assert_eq!(line_breaks("first\r\nsecond\rthird\nfourth"), "first\nsecond\nthird\nfourth");
}

#[test]
fn pasted_line_breaks_of_any_kind_become_one_space_each() {
    assert_eq!(one_line("first\nsecond"), "first second");
    assert_eq!(one_line("first\r\nsecond\rthird"), "first second third");
    assert_eq!(one_line("a\n\nb"), "a  b");
}

#[test]
fn a_field_stays_within_its_rows() {
    let limits = 3..=12;
    assert_eq!(rows_shown(1, Some(&limits)), 3);
    assert_eq!(rows_shown(7, Some(&limits)), 7);
    assert_eq!(rows_shown(40, Some(&limits)), 12);
}

#[test]
fn a_field_without_limits_shows_every_row() {
    assert_eq!(rows_shown(40, None), 40);
}

#[test]
fn a_caret_in_view_leaves_the_scroll_alone() {
    assert_eq!(scroll_to_show(px(40.), px(60.), px(80.), px(100.)), px(40.));
}

#[test]
fn a_caret_above_the_view_scrolls_up_to_it() {
    assert_eq!(scroll_to_show(px(40.), px(20.), px(40.), px(100.)), px(20.));
}

#[test]
fn a_caret_below_the_view_scrolls_down_to_it() {
    assert_eq!(scroll_to_show(px(40.), px(140.), px(160.), px(100.)), px(60.));
}

#[test]
fn a_scroll_stays_within_what_there_is_to_show() {
    assert_eq!(clamp_scroll(px(500.), px(300.), px(100.)), px(200.));
    assert_eq!(clamp_scroll(px(-5.), px(300.), px(100.)), px(0.));
}

#[test]
fn text_that_fits_does_not_scroll() {
    assert_eq!(clamp_scroll(px(50.), px(60.), px(100.)), px(0.));
}

#[test]
fn text_that_fits_has_no_thumb() {
    assert_eq!(thumb(px(0.), px(100.), px(100.)), None);
}

#[test]
fn the_thumb_shows_the_share_in_view_and_where_it_is() {
    assert_eq!(thumb(px(0.), px(400.), px(100.)), Some((px(0.), px(25.))));
    assert_eq!(thumb(px(300.), px(400.), px(100.)), Some((px(75.), px(25.))));
}

#[test]
fn a_very_long_text_keeps_a_thumb_to_see() {
    assert_eq!(thumb(px(0.), px(10_000.), px(100.)).map(|(_, length)| length), Some(px(16.)));
}

#[test]
fn a_view_with_room_keeps_the_margins_around_the_caret() {
    let margins = (px(12.), px(32.));
    assert_eq!(scroll_to_show_with_margins(px(0.), (px(180.), px(16.)), margins, px(200.)), px(28.));
}

#[test]
fn a_view_too_short_for_the_margins_still_shows_the_caret() {
    let (caret, line, view) = (px(300.), px(16.), px(40.));
    let scroll = scroll_to_show_with_margins(px(0.), (caret, line), (px(12.), px(32.)), view);
    assert!(scroll <= caret && caret + line <= scroll + view, "caret at {caret:?} is out of {scroll:?}+{view:?}");
}

#[test]
fn a_view_shorter_than_a_row_follows_the_caret_row() {
    let margins = (px(12.), px(32.));
    assert_eq!(scroll_to_show_with_margins(px(0.), (px(300.), px(16.)), margins, px(10.)), px(306.));
}

#[test]
fn a_text_narrower_than_the_field_never_scrolls_sideways() {
    assert_eq!(sideways(px(0.), (px(50.), px(80.)), px(118.)), px(0.));
}

#[test]
fn a_caret_past_the_right_edge_scrolls_the_text_until_it_shows() {
    assert_eq!(sideways(px(0.), (px(5000.), px(5000.)), px(118.)), px(5000. + 2. - 118.));
}

#[test]
fn going_home_brings_the_start_of_the_text_back() {
    assert_eq!(sideways(px(4884.), (px(0.), px(5000.)), px(118.)), px(0.));
}

#[test]
fn a_shorter_text_gives_back_the_scroll_it_no_longer_needs() {
    assert_eq!(sideways(px(4884.), (px(300.), px(300.)), px(118.)), px(184.));
}

#[test]
fn a_narrower_field_scrolls_until_the_caret_shows() {
    assert_eq!(sideways(px(0.), (px(200.), px(400.)), px(80.)), px(200. + 2. - 80.));
}

#[test]
fn a_scroll_within_what_the_caret_allows_stays_where_it_was() {
    assert_eq!(sideways(px(100.), (px(150.), px(5000.)), px(118.)), px(100.));
}
