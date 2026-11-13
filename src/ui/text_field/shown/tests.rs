use gpui::{Hsla, SharedString, TextRun, font};

use super::{BULLET, Shown};
use crate::ui::text_field::rows::Rows;

fn bullets(content: &str) -> Shown {
    Shown::for_content(&SharedString::from(content.to_string()), true).1
}

fn run(len: usize) -> TextRun {
    TextRun {
        len,
        font: font("any"),
        color: Hsla::default(),
        background_color: None,
        underline: None,
        strikethrough: None,
    }
}

fn cuts(runs: &[TextRun]) -> Vec<(usize, bool)> {
    runs.iter().map(|run| (run.len, run.underline.is_some())).collect()
}

#[test]
fn a_field_that_is_not_masked_maps_every_offset_to_itself() {
    for n in 0..=20 {
        assert_eq!(Shown::Itself.to_shown(n), n);
        assert_eq!(Shown::Itself.to_content(n), n);
    }
    assert_eq!(Shown::Itself.range_to_shown(&(2..7)), 2..7);
}

#[test]
fn a_field_that_is_not_masked_shows_its_text_with_tabs_as_spaces() {
    let (text, shown) = Shown::for_content(&SharedString::from("a\tb"), false);
    assert_eq!((text.as_ref(), shown), ("a b", Shown::Itself));
}

#[test]
fn a_masked_field_shows_one_bullet_per_grapheme() {
    for content in ["a", "ñ", "€", "😀", "🇪🇸", "👨‍👩‍👧", "e\u{301}"] {
        let (text, _) = Shown::for_content(&SharedString::from(content), true);
        assert_eq!(text.as_ref(), BULLET.to_string(), "{content:?}");
    }
    let (text, _) = Shown::for_content(&SharedString::from("a\tb"), true);
    assert_eq!(text.chars().count(), 3);
}

#[test]
fn each_width_of_character_maps_to_the_start_of_its_bullet() {
    let shown = bullets("añ€😀");
    let mapped: Vec<usize> = [0, 1, 3, 6, 10].iter().map(|&at| shown.to_shown(at)).collect();
    assert_eq!(mapped, [0, 3, 6, 9, 12]);
}

#[test]
fn each_bullet_maps_back_to_the_start_of_its_character() {
    let shown = bullets("añ€😀");
    let mapped: Vec<usize> = [0, 3, 6, 9, 12].iter().map(|&at| shown.to_content(at)).collect();
    assert_eq!(mapped, [0, 1, 3, 6, 10]);
}

#[test]
fn a_grapheme_of_several_chars_is_one_bullet_both_ways() {
    let flag = bullets("🇪🇸x");
    assert_eq!((flag.to_shown(8), flag.to_content(3), flag.to_shown(9)), (3, 8, 6));
    let family = bullets("👨‍👩‍👧");
    assert_eq!((family.to_shown(18), family.to_content(3)), (3, 18));
    let accent = bullets("e\u{301}");
    assert_eq!((accent.to_shown(3), accent.to_content(3)), (3, 3));
}

#[test]
fn an_offset_inside_a_grapheme_or_a_character_belongs_to_its_bullet() {
    assert_eq!(bullets("🇪🇸").to_shown(4), 0);
    assert_eq!(bullets("ñ").to_shown(1), 0);
    assert_eq!(bullets("e\u{301}").to_shown(1), 0);
}

#[test]
fn a_shown_offset_inside_a_bullet_maps_to_the_start_of_its_grapheme() {
    assert_eq!(bullets("abc").to_content(4), 1);
}

#[test]
fn an_offset_past_the_end_maps_to_the_end() {
    let shown = bullets("añ");
    assert_eq!((shown.to_shown(100), shown.to_content(100)), (6, 3));
}

#[test]
fn composed_text_in_a_masked_field_underlines_whole_bullets() {
    let runs = bullets("abc´").runs(run(12), Some(&(3..5)));
    assert_eq!(cuts(&runs), [(9, false), (3, true)]);
}

#[test]
fn composed_text_after_wide_characters_stays_within_the_bullets() {
    let runs = bullets("😀😀´").runs(run(9), Some(&(8..10)));
    assert_eq!(cuts(&runs), [(6, false), (3, true)]);
}

#[test]
fn a_plain_field_underlines_its_composed_text_in_place() {
    let runs = Shown::Itself.runs(run(5), Some(&(2..4)));
    assert_eq!(cuts(&runs), [(2, false), (2, true), (1, false)]);
}

#[test]
fn a_marked_range_past_the_text_never_cuts_outside_it() {
    let runs = Shown::Itself.runs(run(3), Some(&(2..9)));
    assert_eq!(cuts(&runs), [(2, false), (1, true)]);
}

#[test]
fn a_masked_row_ends_before_the_bullet_it_wraps_at() {
    let shown = bullets("abcdef");
    let rows = Rows::new([(18, vec![0, 9])]);
    let ends: Vec<_> = (0..2).map(|at| shown.row_ends(rows.get(at).expect("row"), "abcdef")).collect();
    assert_eq!(ends, [(0, 2), (3, 6)]);
}

#[test]
fn a_plain_row_keeps_its_ends() {
    let rows = Rows::new([(16, vec![0, 10])]);
    let ends: Vec<_> =
        (0..2).map(|at| Shown::Itself.row_ends(rows.get(at).expect("row"), "first row second")).collect();
    assert_eq!(ends, [(0, 9), (10, 16)]);
}

#[test]
fn tabs_and_carriage_returns_show_as_spaces_of_the_same_length() {
    assert_eq!(super::shown_without_controls("a\tb\r\nc").as_deref(), Some("a b \nc"));
}

#[test]
fn text_without_controls_is_shown_as_it_is() {
    assert_eq!(super::shown_without_controls("a b\nc"), None);
}
