use gpui::{FontWeight, HighlightStyle, TextRun, TextStyle};

use super::{ELLIPSIS, ending, kept, one_line, runs};

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
