use std::ops::Range;

use gpui::{Bounds, HighlightStyle, Hsla, point, px, size};

use super::{
    Caret, Edge, Lit, Press, Selection, Span, Zone, bar_origin, edge_at, host, lines_label, off_centre, off_edge,
    overlay, reference, selected_chars, row_at, row_from, row_near, word_at,
};

fn caret(spot: usize, at: usize) -> Caret<usize> {
    Caret { spot, at }
}

fn chars(anchor: (usize, usize), head: (usize, usize)) -> Selection<usize> {
    Selection::Chars { anchor: caret(anchor.0, anchor.1), head: caret(head.0, head.1) }
}

fn lines(anchor: usize, head: usize) -> Selection<usize> {
    Selection::Lines { anchor, head }
}

fn span(top: f32) -> Span {
    (px(top), px(top), px(top + 18.))
}

fn row(top: f32, height: f32) -> Bounds<gpui::Pixels> {
    Bounds::new(point(px(40.), px(top)), size(px(300.), px(height)))
}

#[test]
fn a_range_of_lines_lights_every_line_between_its_ends_either_way() {
    for selection in [lines(2, 4), lines(4, 2)] {
        assert_eq!(selection.lit(1, "x"), Lit::Nothing);
        assert_eq!(selection.lit(2, "x"), Lit::Line);
        assert_eq!(selection.lit(4, "x"), Lit::Line);
        assert_eq!(selection.lit(5, "x"), Lit::Nothing);
    }
}

#[test]
fn characters_on_one_line_light_only_that_range_either_way() {
    for selection in [chars((3, 2), (3, 5)), chars((3, 5), (3, 2))] {
        assert_eq!(selection.lit(3, "abcdefg"), Lit::Chars { range: 2..5, line_break: false });
        assert_eq!(selection.lit(2, "abcdefg"), Lit::Nothing);
    }
}

#[test]
fn characters_across_lines_run_from_the_first_caret_to_the_last() {
    let selection = chars((5, 1), (3, 2));
    assert_eq!(selection.lit(3, "abcd"), Lit::Chars { range: 2..4, line_break: true });
    assert_eq!(selection.lit(4, "abcd"), Lit::Chars { range: 0..4, line_break: true });
    assert_eq!(selection.lit(5, "abcd"), Lit::Chars { range: 0..1, line_break: false });
}

#[test]
fn a_caret_inside_a_character_or_past_the_end_lights_whole_characters() {
    let selection = chars((0, 2), (0, 99));
    assert_eq!(selection.lit(0, "añb"), Lit::Chars { range: 1..4, line_break: false });
}

#[test]
fn a_last_line_with_nothing_selected_is_not_covered() {
    let selection = chars((1, 2), (3, 0));
    assert!(selection.lit(2, "").covers());
    assert!(!selection.lit(3, "abc").covers());
    assert_eq!(selection.covered(), (1, 2));
}

#[test]
fn an_empty_line_inside_a_selection_draws_its_break_and_lights_no_character() {
    let lit = chars((1, 0), (3, 1)).lit(2, "");
    assert_eq!(lit.drawn_range(), None);
    assert!(lit.draws_break());
}

#[test]
fn nothing_selected_on_a_line_draws_nothing() {
    let lit = chars((1, 2), (3, 0)).lit(3, "abc");
    assert_eq!(lit.drawn_range(), None);
    assert!(!lit.draws_break());
}

#[test]
fn a_line_the_selection_runs_past_draws_its_characters_and_its_break() {
    let lit = chars((0, 1), (2, 2)).lit(0, "abc");
    assert_eq!(lit.drawn_range(), Some(1..3));
    assert!(lit.draws_break());
}

#[test]
fn from_a_line_s_end_to_the_next_line_s_start_only_the_break_is_drawn() {
    let selection = chars((0, 3), (1, 0));
    let first = selection.lit(0, "abc");
    assert_eq!((first.drawn_range(), first.draws_break()), (None, true));
    let second = selection.lit(1, "def");
    assert_eq!((second.drawn_range(), second.draws_break()), (None, false));
}

#[test]
fn the_last_line_of_a_selection_never_draws_a_break() {
    for selection in [chars((0, 1), (2, 2)), chars((2, 2), (0, 1))] {
        let lit = selection.lit(2, "abcd");
        assert_eq!((lit.drawn_range(), lit.draws_break()), (Some(0..2), false));
    }
}

#[test]
fn whole_lines_draw_no_characters_and_no_break() {
    let lit = lines(0, 2).lit(1, "abc");
    assert_eq!((lit.drawn_range(), lit.draws_break()), (None, false));
}

#[test]
fn a_shown_line_before_lines_not_shown_draws_its_break() {
    let lit = chars((1, 1), (3, 2)).lit(1, "de");
    assert_eq!((lit.drawn_range(), lit.draws_break()), (Some(1..2), true));
}

#[test]
fn painting_lights_only_selected_characters() {
    assert!(chars((1, 0), (3, 1)).lit(2, "").painted(Vec::new()).is_empty());
    let lit = chars((0, 1), (2, 0)).lit(0, "abc");
    assert_eq!(lit.painted(Vec::new()), vec![(1..3, selected_chars())]);
    let styles = vec![(0..2, HighlightStyle::default())];
    assert_eq!(Lit::Nothing.painted(styles.clone()), styles);
}

#[test]
fn the_parts_of_a_selection_are_what_each_line_gives_in_order() {
    let text = ["alpha", "", "gamma", "delta"];
    let numbered = || text.iter().enumerate().map(|(i, line)| (i, *line));
    assert_eq!(chars((0, 3), (2, 2)).parts(numbered()), vec![(0, 3..5), (1, 0..0), (2, 0..2)]);
    assert_eq!(chars((0, 3), (3, 0)).parts(numbered()), vec![(0, 3..5), (1, 0..0), (2, 0..5)]);
    assert_eq!(lines(1, 2).parts(numbered()), vec![(1, 0..0), (2, 0..5)]);
}

#[test]
fn only_a_selection_inside_one_line_reads_as_its_characters() {
    assert_eq!(chars((3, 5), (3, 2)).one_line_chars(), Some((3, 2..5)));
    assert_eq!(chars((3, 5), (4, 2)).one_line_chars(), None);
    assert_eq!(lines(3, 3).one_line_chars(), None);
}

#[test]
fn a_selection_past_the_end_of_a_file_that_shrank_is_dropped() {
    assert_eq!(lines(5, 7).fit(5), None);
    assert_eq!(lines(5, 7).fit(0), None);
}

#[test]
fn a_selection_that_reaches_past_the_end_stops_at_the_last_line() {
    assert_eq!(lines(2, 7).fit(5), Some(lines(2, 4)));
    assert_eq!(chars((2, 1), (7, 3)).fit(5), Some(chars((2, 1), (4, 3))));
}

#[test]
fn a_press_on_the_numbers_selects_its_line_at_once() {
    let press = Press::down(None, caret(4, 0), Zone::Numbers, false);
    assert_eq!(press.at_once(), Some(Selection::line(4)));
}

#[test]
fn a_press_on_the_code_leaves_the_selection_until_it_moves_or_ends() {
    let before = Some(lines(1, 2));
    let press = Press::down(before, caret(4, 3), Zone::Code, false);
    assert_eq!(press.at_once(), before);
}

#[test]
fn a_click_on_the_code_selects_its_line() {
    let press = Press::down(Some(lines(1, 2)), caret(4, 3), Zone::Code, false);
    assert_eq!(press.released(press.at_once()), Some(Selection::line(4)));
}

#[test]
fn a_click_on_the_only_selected_line_clears_it() {
    for zone in [Zone::Code, Zone::Numbers] {
        let press = Press::down(Some(Selection::line(4)), caret(4, 3), zone, false);
        assert_eq!(press.released(press.at_once()), None);
    }
}

#[test]
fn a_drag_on_the_numbers_selects_whole_lines_either_way() {
    let mut press = Press::down(None, caret(4, 0), Zone::Numbers, false);
    assert_eq!(press.dragged(caret(7, 2)), lines(4, 7));
    assert_eq!(press.dragged(caret(2, 2)), lines(4, 2));
    assert_eq!(press.released(Some(lines(4, 2))), Some(lines(4, 2)));
}

#[test]
fn a_drag_inside_a_line_selects_its_characters() {
    let mut press = Press::down(None, caret(4, 2), Zone::Code, false);
    assert_eq!(press.dragged(caret(4, 6)), chars((4, 2), (4, 6)));
}

#[test]
fn a_drag_across_lines_runs_from_the_press_to_the_pointer() {
    let mut press = Press::down(None, caret(4, 2), Zone::Code, false);
    assert_eq!(press.dragged(caret(6, 1)), chars((4, 2), (6, 1)));
}

#[test]
fn a_drag_back_to_the_press_falls_back_to_its_line() {
    let mut press = Press::down(None, caret(4, 2), Zone::Code, false);
    press.dragged(caret(4, 6));
    assert_eq!(press.dragged(caret(4, 2)), Selection::line(4));
}

#[test]
fn a_finished_drag_keeps_what_it_selected() {
    let mut press = Press::down(None, caret(4, 2), Zone::Code, false);
    let dragged = press.dragged(caret(4, 6));
    assert_eq!(press.released(Some(dragged)), Some(dragged));
}

#[test]
fn a_shift_click_makes_a_range_from_where_the_selection_started() {
    let press = Press::down(Some(lines(2, 3)), caret(9, 1), Zone::Code, true);
    assert_eq!(press.at_once(), Some(lines(2, 9)));
    assert_eq!(press.released(press.at_once()), Some(lines(2, 9)));
}

#[test]
fn a_shift_click_on_a_character_selection_goes_on_from_its_start_as_lines() {
    let press = Press::down(Some(chars((3, 4), (3, 8))), caret(6, 1), Zone::Numbers, true);
    assert_eq!(press.at_once(), Some(lines(3, 6)));
}

#[test]
fn a_shift_click_with_nothing_selected_selects_its_line() {
    let press = Press::down(None, caret(6, 1), Zone::Code, true);
    assert_eq!(press.released(press.at_once()), Some(Selection::line(6)));
}

#[test]
fn a_double_click_selects_the_word_and_stays_selected() {
    let (press, selected) = Press::on_word(Some(Selection::line(4)), 4, 2..6);
    assert_eq!(selected, chars((4, 2), (4, 6)));
    assert_eq!(press.released(Some(selected)), Some(selected));
}

#[test]
fn a_word_is_found_under_the_caret() {
    assert_eq!(word_at("let total = 1;", 6), 4..9);
    assert_eq!(word_at("let total = 1;", 0), 0..3);
}

#[test]
fn a_caret_just_after_a_word_still_takes_that_word() {
    assert_eq!(word_at("let total = 1;", 9), 4..9);
}

#[test]
fn a_caret_on_punctuation_between_spaces_takes_it_alone() {
    assert_eq!(word_at("a = b", 2), 2..3);
}

#[test]
fn a_caret_past_the_end_takes_the_last_word() {
    assert_eq!(word_at("let total", 9), 4..9);
    assert_eq!(word_at("", 0), 0..0);
}

#[test]
fn the_pointer_past_the_body_s_edges_scrolls() {
    let body = row(100., 400.);
    assert_eq!(edge_at(px(90.), body), Some(Edge::Top));
    assert_eq!(edge_at(px(900.), body), Some(Edge::Bottom));
}

#[test]
fn the_pointer_inside_the_body_does_not_scroll_even_on_its_first_or_last_row() {
    let body = row(100., 400.);
    assert_eq!(edge_at(px(101.), body), None);
    assert_eq!(edge_at(px(499.), body), None);
}

#[test]
fn a_press_finds_only_the_row_it_landed_on() {
    let rows = [(1, row(0., 18.)), (2, row(18., 36.)), (4, row(90., 18.))];
    assert_eq!(row_at(rows.into_iter(), px(20.)), Some(2));
    assert_eq!(row_at(rows.into_iter(), px(60.)), None);
    assert_eq!(row_at(rows.into_iter(), px(200.)), None);
}

#[test]
fn a_drag_takes_the_last_row_at_or_above_the_pointer_else_the_first() {
    let rows = [(1, row(10., 18.)), (2, row(28., 18.)), (4, row(90., 18.))];
    assert_eq!(row_near(rows.into_iter(), px(0.)), Some(1));
    assert_eq!(row_near(rows.into_iter(), px(60.)), Some(2));
    assert_eq!(row_near(rows.into_iter(), px(900.)), Some(4));
}

#[test]
fn the_bar_floats_by_the_head_when_it_is_drawn() {
    assert_eq!(host(lines(3, 6), [2, 3, 4, 5, 6, 7].into_iter()), Some(6));
    assert_eq!(host(lines(6, 3), [2, 3, 4, 5, 6, 7].into_iter()), Some(3));
}

#[test]
fn the_bar_floats_by_the_nearest_drawn_line_when_the_head_is_scrolled_away() {
    assert_eq!(host(lines(3, 90), [10, 11, 12].into_iter()), Some(12));
    assert_eq!(host(lines(90, 3), [10, 11, 12].into_iter()), Some(10));
}

#[test]
fn no_bar_shows_with_the_whole_selection_scrolled_away() {
    assert_eq!(host(lines(3, 6), [10, 11, 12].into_iter()), None);
}

#[test]
fn the_bar_sits_under_its_line_at_the_right_edge() {
    let room = Bounds::new(point(px(0.), px(0.)), size(px(500.), px(400.)));
    let origin = bar_origin(row(100., 18.), size(px(120.), px(26.)), room);
    assert_eq!(origin, point(px(372.), px(122.)));
}

#[test]
fn the_bar_goes_above_its_line_when_there_is_no_room_below() {
    let room = Bounds::new(point(px(0.), px(0.)), size(px(500.), px(400.)));
    let origin = bar_origin(row(380., 18.), size(px(120.), px(26.)), room);
    assert_eq!(origin.y, px(350.));
}

#[test]
fn the_bar_never_leaves_the_room() {
    let room = Bounds::new(point(px(0.), px(0.)), size(px(100.), px(30.)));
    let origin = bar_origin(row(4., 18.), size(px(120.), px(26.)), room);
    assert_eq!(origin, point(px(8.), px(8.)));
}

#[test]
fn a_reference_names_one_line_or_a_range() {
    assert_eq!(reference("src/a.rs", (4, 5)), "src/a.rs:4-5 ");
    assert_eq!(reference("src/a.rs", (12, 12)), "src/a.rs:12 ");
}

#[test]
fn a_label_names_one_line_or_a_range() {
    assert_eq!(lines_label((4, 5)), "lines 4-5");
    assert_eq!(lines_label((12, 12)), "line 12");
}

fn only(range: Range<usize>) -> Vec<Range<usize>> {
    vec![range]
}

fn red() -> HighlightStyle {
    HighlightStyle { color: Some(Hsla::red()), ..HighlightStyle::default() }
}

fn lit_white() -> HighlightStyle {
    HighlightStyle { background_color: Some(Hsla::white()), ..HighlightStyle::default() }
}

#[test]
fn overlay_splits_styles_under_lit_ranges() {
    let out = overlay(vec![(0..6, red())], &only(4..8), lit_white());
    let ranges: Vec<_> = out.iter().map(|(r, _)| r.clone()).collect();
    assert_eq!(ranges, vec![0..4, 4..6, 6..8]);
    assert_eq!(out[1].1.color, red().color);
    assert_eq!(out[1].1.background_color, Some(Hsla::white()));
    assert_eq!(out[2].1.color, None);
}

#[test]
fn overlay_of_nothing_leaves_the_styles() {
    assert_eq!(overlay(vec![(0..2, red())], &[], lit_white()).len(), 1);
}

#[test]
fn overlay_can_change_the_text_s_colour_too() {
    let paint = HighlightStyle { color: Some(Hsla::white()), ..lit_white() };
    let out = overlay(vec![(0..6, red())], &only(0..6), paint);
    assert_eq!(out[0].1.color, Some(Hsla::white()));
}

#[test]
fn a_second_overlay_goes_over_the_first() {
    let once = overlay(vec![(0..6, red())], &[1..3, 4..5], lit_white());
    let blue = HighlightStyle { background_color: Some(Hsla::blue()), ..HighlightStyle::default() };
    let twice = overlay(once, &only(2..5), blue);
    let backgrounds: Vec<_> = twice.iter().map(|(r, s)| (r.clone(), s.background_color)).collect();
    assert_eq!(
        backgrounds,
        vec![
            (0..1, None),
            (1..2, Some(Hsla::white())),
            (2..3, Some(Hsla::blue())),
            (3..4, Some(Hsla::blue())),
            (4..5, Some(Hsla::blue())),
            (5..6, None),
        ]
    );
}

#[test]
fn overlay_keeps_a_gap_between_styles_unstyled() {
    let out = overlay(vec![(0..1, red()), (5..6, red())], &only(8..9), lit_white());
    let ranges: Vec<_> = out.iter().map(|(r, _)| r.clone()).collect();
    assert_eq!(ranges, vec![0..1, 5..6, 8..9]);
}

#[test]
fn a_row_inside_the_body_needs_no_scroll() {
    assert_eq!(off_centre(px(100.), row(0., 400.), px(0.)), None);
}

#[test]
fn a_row_cut_by_the_bottom_of_the_body_is_as_far_as_the_centre_says() {
    assert_eq!(off_centre(px(390.), row(0., 400.), px(0.)), Some(px(190.)));
}

#[test]
fn a_row_above_the_body_scrolls_back() {
    assert_eq!(off_centre(px(-50.), row(0., 400.), px(0.)), Some(px(-250.)));
}

#[test]
fn a_row_under_the_find_field_counts_as_out_of_view() {
    assert_eq!(off_centre(px(30.), row(0., 400.), px(56.)), Some(px(-198.)));
    assert_eq!(off_centre(px(56.), row(0., 400.), px(56.)), None);
}

#[test]
fn a_selection_whose_head_is_on_its_anchor_is_empty() {
    assert!(chars((1, 2), (1, 2)).is_empty());
    assert!(!chars((1, 2), (1, 3)).is_empty());
    assert!(!lines(2, 2).is_empty());
}

#[test]
fn the_head_caret_of_whole_lines_is_the_start_of_their_head_line() {
    assert_eq!(lines(1, 4).head_caret(), caret(4, 0));
    assert_eq!(chars((1, 2), (3, 5)).head_caret(), caret(3, 5));
}

#[test]
fn a_caret_row_with_a_row_to_spare_needs_no_scroll() {
    assert_eq!(off_edge(span(100.), row(0., 400.), px(0.)), None);
}

#[test]
fn a_caret_row_near_the_bottom_scrolls_just_enough_to_keep_a_row_under_it() {
    assert_eq!(off_edge(span(370.), row(0., 400.), px(0.)), Some(px(6.)));
    assert_eq!(off_edge(span(390.), row(0., 400.), px(0.)), Some(px(26.)));
}

#[test]
fn a_caret_row_above_the_top_scrolls_back_just_enough_to_keep_a_row_over_it() {
    assert_eq!(off_edge(span(-50.), row(0., 400.), px(0.)), Some(px(-68.)));
}

#[test]
fn a_caret_row_under_the_find_field_counts_as_out_of_view() {
    assert_eq!(off_edge(span(30.), row(0., 400.), px(56.)), Some(px(-44.)));
    assert_eq!(off_edge(span(74.), row(0., 400.), px(56.)), None);
}

#[test]
fn the_first_row_shown_is_the_first_reaching_below_the_top() {
    let rows = || [(1, row(-30., 18.)), (2, row(-12., 18.)), (3, row(6., 18.))].into_iter();
    assert_eq!(row_from(rows(), px(0.)), Some(2));
    assert_eq!(row_from(rows(), px(10.)), Some(3));
}

#[test]
fn a_line_of_four_rows_near_the_bottom_scrolls_to_show_its_end_and_a_row_under_it() {
    assert_eq!(off_edge((px(340.), px(340.), px(412.)), row(0., 400.), px(0.)), Some(px(30.)));
}

#[test]
fn a_caret_in_view_inside_a_line_taller_than_the_view_leaves_the_view_alone() {
    assert_eq!(off_edge((px(-500.), px(100.), px(2000.)), row(0., 400.), px(0.)), None);
}

#[test]
fn a_caret_row_near_the_bottom_of_a_line_taller_than_the_view_scrolls_a_row() {
    assert_eq!(off_edge((px(-500.), px(380.), px(2000.)), row(0., 400.), px(0.)), Some(px(16.)));
}

#[test]
fn a_span_shorter_than_a_row_counts_as_one_row() {
    assert_eq!(off_edge((px(370.), px(370.), px(370.)), row(0., 400.), px(0.)), Some(px(6.)));
}

#[test]
fn a_head_on_the_last_row_of_a_line_keeps_the_whole_line_and_a_row_above_it() {
    assert_eq!(off_edge((px(-36.), px(0.), px(18.)), row(0., 400.), px(0.)), Some(px(-54.)));
}

fn copied(selection: Selection<usize>, shown: &[(usize, &str)]) -> String {
    let own = |spot: usize, range: Range<usize>| {
        shown.iter().find(|(ix, _)| *ix == spot).and_then(|(_, line)| line.get(range)).unwrap_or_default()
    };
    selection.text(shown.iter().copied(), own)
}

#[test]
fn characters_inside_one_line_copy_only_them() {
    assert_eq!(copied(chars((0, 1), (0, 3)), &[(0, "abcd")]), "bc");
}

#[test]
fn characters_across_lines_copy_the_breaks_between_them() {
    let shown = [(0, "abc"), (1, "de"), (2, "fgh")];
    assert_eq!(copied(chars((0, 2), (2, 1)), &shown), "c\nde\nf");
}

#[test]
fn characters_ending_at_a_line_s_start_copy_the_break_before_it() {
    let shown = [(0, "abc"), (1, "de")];
    assert_eq!(copied(chars((0, 1), (1, 0)), &shown), "bc\n");
    assert_eq!(copied(chars((1, 0), (0, 1)), &shown), "bc\n");
}

#[test]
fn from_a_line_s_end_to_the_next_line_s_start_copies_only_its_break() {
    assert_eq!(copied(chars((0, 3), (1, 0)), &[(0, "abc"), (1, "de")]), "\n");
}

#[test]
fn an_empty_line_inside_the_selection_copies_its_break_too() {
    assert_eq!(copied(chars((0, 1), (2, 0)), &[(0, "abc"), (1, ""), (2, "x")]), "bc\n\n");
}

#[test]
fn whole_lines_copy_no_break_after_the_last() {
    let shown = [(0, "abc"), (1, "de"), (2, "fgh")];
    assert_eq!(copied(lines(0, 1), &shown), "abc\nde");
    assert_eq!(copied(lines(0, 9), &shown), "abc\nde\nfgh");
}

#[test]
fn lines_not_shown_are_not_copied_and_the_last_shown_keeps_its_break() {
    let shown = [(0, "abc"), (1, "de"), (3, "fgh")];
    assert_eq!(copied(chars((1, 1), (3, 0)), &shown), "e\n");
    assert_eq!(copied(chars((1, 1), (2, 1)), &shown), "e\n");
    assert_eq!(copied(lines(1, 2), &shown), "de");
}

#[test]
fn each_part_comes_from_the_line_s_own_text() {
    let shown = [(0, "abc"), (1, "de")];
    let own = ["ABC", "DE"];
    let text =
        chars((0, 1), (1, 0)).text(shown.iter().copied(), |spot, range| own[spot].get(range).unwrap_or_default());
    assert_eq!(text, "BC\n");
}

#[test]
fn a_selection_of_nothing_copies_nothing() {
    assert_eq!(copied(chars((0, 2), (0, 2)), &[(0, "abc")]), "");
}
