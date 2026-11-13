use gpui::{AvailableSpace, Bounds, FontWeight, HighlightStyle, Hsla, TextRun, TextStyle, point, px, size};

use super::{Cut, ELLIPSIS, caret_in, end_in, ending, fits, kept, natural, one_line, runs};

fn bold() -> HighlightStyle {
    HighlightStyle { font_weight: Some(FontWeight::BOLD), ..Default::default() }
}

fn shape(runs: &[TextRun]) -> Vec<(usize, FontWeight)> {
    runs.iter().map(|run| (run.len, run.font.weight)).collect()
}

#[test]
fn a_text_without_highlights_is_one_run() {
    let runs = runs("cornercase", &TextStyle::default(), &[]);
    assert_eq!(shape(&runs), vec![(10, FontWeight::NORMAL)]);
}

#[test]
fn a_highlight_splits_the_text_into_runs() {
    let runs = runs("a match here", &TextStyle::default(), &[(2..7, bold())]);
    assert_eq!(shape(&runs), vec![(2, FontWeight::NORMAL), (5, FontWeight::BOLD), (5, FontWeight::NORMAL)]);
}

#[test]
fn highlights_past_the_end_or_overlapping_are_clamped() {
    let runs = runs("abcdef", &TextStyle::default(), &[(1..3, bold()), (2..4, bold()), (5..99, bold())]);
    assert_eq!(runs.iter().map(|run| run.len).sum::<usize>(), 6);
    assert_eq!(shape(&runs)[0], (1, FontWeight::NORMAL));
}

#[test]
fn the_cut_keeps_whole_characters() {
    assert_eq!(kept("añb", 2), 1);
}

#[test]
fn the_cut_drops_the_spaces_before_the_ellipsis() {
    assert_eq!(kept("long line of code", 13), 12);
}

#[test]
fn the_ending_keeps_the_styles_up_to_the_cut_then_the_ellipsis() {
    let style = TextStyle::default();
    let highlighted = style.clone().highlight(bold());
    let runs = vec![style.to_run(3), highlighted.to_run(4), style.to_run(5)];
    let ended = ending(&runs, 5, style.to_run(ELLIPSIS.len()));
    assert_eq!(shape(&ended), vec![(3, FontWeight::NORMAL), (2, FontWeight::BOLD), (3, FontWeight::NORMAL)]);
}

#[test]
fn line_breaks_and_tabs_read_as_spaces() {
    assert_eq!(one_line("one\ntwo\tthree\r").text.as_ref(), "one two three ");
}

#[test]
fn the_ellipsis_is_lit_when_something_lit_is_hidden_behind_it() {
    let style = TextStyle::default();
    let lit = HighlightStyle { background_color: Some(Hsla::white()), ..Default::default() };
    let runs = vec![style.to_run(9), style.clone().highlight(lit).to_run(2), style.to_run(1)];
    let ended = ending(&runs, 5, style.to_run(ELLIPSIS.len()));
    assert_eq!(ended.last().and_then(|mark| mark.background_color), Some(Hsla::white()));
}

#[test]
fn the_ellipsis_stays_plain_when_only_what_shows_is_lit() {
    let style = TextStyle::default();
    let lit = HighlightStyle { background_color: Some(Hsla::white()), ..Default::default() };
    let runs = vec![style.clone().highlight(lit).to_run(5), style.to_run(7)];
    let ended = ending(&runs, 5, style.to_run(ELLIPSIS.len()));
    assert_eq!(ended.last().and_then(|mark| mark.background_color), None);
}

#[test]
fn a_line_that_fits_takes_the_closest_character() {
    assert_eq!(caret_in(None, 20, px(70.), 10), 10);
}

#[test]
fn the_ellipsis_stands_for_the_hidden_end_of_the_line() {
    let cut = Some(Cut { kept: 12, mark_x: px(84.) });
    assert_eq!(caret_in(cut, 200, px(90.), 13), 200);
}

#[test]
fn before_the_ellipsis_a_cut_line_stops_at_what_it_kept() {
    let cut = Some(Cut { kept: 12, mark_x: px(84.) });
    assert_eq!(caret_in(cut, 200, px(83.), 13), 12);
    assert_eq!(caret_in(cut, 200, px(14.), 2), 2);
}

#[test]
fn a_plain_line_asks_for_nothing_at_its_least() {
    assert_eq!(natural(px(120.), AvailableSpace::MinContent, None), px(0.));
}

#[test]
fn a_line_with_a_widest_width_keeps_its_own_width_at_its_least() {
    assert_eq!(natural(px(120.), AvailableSpace::MinContent, Some(px(240.))), px(120.));
}

#[test]
fn a_line_longer_than_its_widest_width_asks_for_the_widest() {
    let widest = Some(px(240.));
    assert_eq!(natural(px(5000.), AvailableSpace::MinContent, widest), px(240.));
    assert_eq!(natural(px(5000.), AvailableSpace::MaxContent, widest), px(240.));
}

#[test]
fn a_box_narrower_than_the_widest_width_still_wins() {
    assert_eq!(natural(px(5000.), AvailableSpace::Definite(px(100.)), Some(px(240.))), px(100.));
}

fn box_at(width: f32) -> Bounds<gpui::Pixels> {
    Bounds::new(point(px(40.), px(0.)), size(px(width), px(18.)))
}

#[test]
fn a_line_that_is_not_cut_ends_where_its_text_does() {
    assert_eq!(end_in(false, box_at(300.), px(70.)), px(110.));
}

#[test]
fn a_cut_line_ends_at_the_box_s_edge_past_its_ellipsis() {
    assert_eq!(end_in(true, box_at(300.), px(900.)), px(340.));
}

#[test]
fn a_line_within_its_box_s_last_pixel_is_not_cut() {
    assert!(fits(px(300.5), px(300.25)));
    assert!(!fits(px(301.5), px(300.25)));
}
