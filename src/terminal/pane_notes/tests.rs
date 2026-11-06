use gpui::{point, px, size};

use super::*;
use crate::render::layout::rect;

fn scrolled(total: u64, offset: u64, rows: u64) -> PaneGrid {
    let mut pane = PaneGrid { pane: 1, ..Default::default() };
    pane.scroll.total = total;
    pane.scroll.offset = offset;
    pane.scroll.rows = rows;
    pane
}

fn held(count: u32, draft: bool) -> PaneInfo {
    PaneInfo { held_sends: count, draft, ..Default::default() }
}

#[test]
fn the_button_says_how_many_lines_are_below() {
    assert_eq!(more_text(30), "↓ 30 more");
}

#[test]
fn the_button_goes_back_to_the_bottom_with_scroll() {
    assert_eq!(bottom_args(9), ["scroll", "--pane", "9", "--bottom"]);
}

#[test]
fn a_message_held_behind_a_draft_goes_after_yours() {
    assert_eq!(held_text(1, true), "1 message waiting · goes after yours");
    assert_eq!(held_text(3, true), "3 messages waiting · goes after yours");
}

#[test]
fn a_message_held_with_no_draft_goes_out_in_a_moment() {
    assert_eq!(held_text(1, false), "1 message waiting · goes out in a moment");
}

#[test]
fn the_band_names_what_waits_when_it_all_fits() {
    let (text, waits) = band_texts("1 message waiting", 200).expect("a band");
    assert_eq!(text, "1 message waiting");
    assert_eq!(waits, Some(WAITS));
    assert!(WAITS.ends_with("usecornercase/cornercase-terminal#676"));
}

#[test]
fn a_narrow_band_drops_what_waits_before_its_message() {
    let (text, waits) = band_texts("1 message waiting", 30).expect("a band");
    assert_eq!(text, "1 message waiting");
    assert_eq!(waits, None);
}

#[test]
fn a_narrower_band_cuts_its_message() {
    let (text, _) = band_texts("1 message waiting", 10).expect("a band");
    assert_eq!(text, "1 message…");
}

#[test]
fn no_room_at_all_draws_no_band() {
    assert!(band_texts("1 message waiting", LEAST_TEXT - 1).is_none());
}

#[test]
fn a_pane_at_the_bottom_holding_nothing_has_no_notes() {
    assert!(notes_of(&scrolled(402, 354, 48), Some(&held(0, true)), false).is_none());
}

#[test]
fn a_pane_scrolled_up_counts_the_lines_below_it() {
    let notes = notes_of(&scrolled(402, 324, 48), None, false).expect("notes");
    assert_eq!(notes.below, 30);
    assert!(notes.held.is_none());
}

#[test]
fn a_pane_holding_a_message_says_so_at_the_bottom_too() {
    let notes = notes_of(&scrolled(48, 0, 48), Some(&held(2, false)), false).expect("notes");
    assert_eq!(notes.below, 0);
    assert_eq!(notes.held.as_deref(), Some("2 messages waiting · goes out in a moment"));
}

#[test]
fn notes_fade_with_an_inactive_pane() {
    let notes = notes_of(&scrolled(402, 324, 48), None, true).expect("notes");
    assert!((notes.opacity - INACTIVE).abs() < f32::EPSILON);
}

#[test]
fn the_band_sits_in_the_bottom_padding_and_covers_no_cell() {
    let card = rect(100., 100., 400., 300.);
    let band = band_bounds(card, px(0.));
    assert_eq!(band.origin, point(px(100. + SIDE), px(400. - LIFT - HEIGHT)));
    assert_eq!(band.size, size(px(400. - 2. * SIDE), px(HEIGHT)));
}

#[test]
fn the_band_leaves_room_for_the_account_chips() {
    let band = band_bounds(rect(0., 0., 400., 300.), px(100.));
    assert!((f32::from(band.size.width) - (400. - 2. * SIDE - 100. - GAP)).abs() < f32::EPSILON);
}

#[test]
fn a_card_too_narrow_for_the_chips_leaves_the_band_no_width() {
    assert_eq!(band_bounds(rect(0., 0., 100., 300.), px(200.)).size.width, px(0.));
}

#[test]
fn the_button_sits_at_the_end_of_the_last_row() {
    let at = more_origin(rect(10., 500., 600., 17.), size(px(80.), px(17.)));
    assert_eq!(at, point(px(530.), px(500.)));
}

#[test]
fn a_button_wider_than_its_pane_stays_at_the_pane_s_start() {
    let at = more_origin(rect(10., 500., 60., 17.), size(px(80.), px(17.)));
    assert_eq!(at, point(px(10.), px(500.)));
}
