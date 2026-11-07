use gpui::{point, px, size};

use super::*;
use crate::model::{PaneInfo, Project, Tab, Workspace};
use crate::render::layout::rect;
use crate::test_util::fixture;

fn scrolled(total: u64, offset: u64, rows: u64) -> PaneGrid {
    let mut pane = PaneGrid { pane: 1, ..Default::default() };
    pane.scroll.total = total;
    pane.scroll.offset = offset;
    pane.scroll.rows = rows;
    pane
}

fn session_of(panes: Vec<PaneInfo>) -> Session {
    let tabs = vec![Tab { panes, ..Default::default() }];
    let workspaces = vec![Workspace { tabs, ..Default::default() }];
    Session { projects: vec![Project { workspaces, ..Default::default() }], ..Default::default() }
}

fn holding(id: u64, count: u32) -> PaneInfo {
    PaneInfo { id, held_sends: count, ..Default::default() }
}

fn agent(id: u64, name: &str) -> PaneInfo {
    PaneInfo { id, agent: Some(name.to_string()), ..Default::default() }
}

fn bar(text: &str, queued: bool, draft: bool) -> Held {
    Held { text: text.to_string(), queued, draft, running: Vec::new() }
}

const LONG: &str = "1 message waiting from claude (pane 9) · goes after yours";

#[test]
fn the_button_says_how_many_lines_are_below() {
    assert_eq!(more_text(30), "↓ 30 more");
}

#[test]
fn the_button_goes_back_to_the_bottom_with_scroll() {
    assert_eq!(bottom_args(9), ["scroll", "--pane", "9", "--bottom"]);
}

#[test]
fn a_message_from_nowhere_names_no_sender() {
    assert_eq!(held_text(1, None, false), "1 message waiting · goes after yours");
    assert_eq!(held_text(3, None, false), "3 messages waiting · goes after yours");
}

#[test]
fn a_message_names_the_pane_that_sent_it() {
    assert_eq!(
        held_text(1, Some("claude (pane 9)"), false),
        "1 message waiting from claude (pane 9) · goes after yours"
    );
}

#[test]
fn several_messages_name_the_sender_of_the_first() {
    let text = held_text(3, Some("claude (pane 9)"), false);
    assert_eq!(text, "3 messages waiting, the first from claude (pane 9) · goes after yours");
}

#[test]
fn a_chosen_wait_turn_says_it_is_queued() {
    assert_eq!(held_text(1, Some("claude (pane 9)"), true), "queued: goes right after yours");
    assert_eq!(held_text(2, None, true), "2 queued: the first goes right after yours");
}

#[test]
fn the_sender_goes_by_the_name_the_window_gives_it() {
    let sender =
        PaneInfo { id: 5, agent: Some("codex".to_string()), program: Some("node".to_string()), ..Default::default() };
    let held = PaneInfo { held_from: Some(5), ..holding(9, 1) };
    let held = held_of(Some(&session_of(vec![sender, held])), 9).expect("a bar");
    assert_eq!(held.text, "1 message waiting from codex (pane 5) · goes after yours");
}

#[test]
fn a_sender_since_closed_keeps_its_id() {
    let held = PaneInfo { held_from: Some(40), ..holding(9, 1) };
    let held = held_of(Some(&session_of(vec![held])), 9).expect("a bar");
    assert_eq!(held.text, "1 message waiting from pane 40 · goes after yours");
}

#[test]
fn a_machine_s_pane_is_named_from_that_machine_s_session() {
    let here = session_of(vec![agent(5, "claude")]);
    let held = PaneInfo { held_from: Some(5), ..holding(9, 1) };
    let there = session_of(vec![agent(5, "codex"), held]);
    assert!(held_of(Some(&here), 9).is_none());
    let held = held_of(Some(&there), 9).expect("a bar");
    assert_eq!(held.text, "1 message waiting from codex (pane 5) · goes after yours");
}

#[test]
fn a_view_line_says_who_sent_it_and_that_it_waits_its_turn() {
    let session: Session = serde_json::from_str(fixture!("session/held_queued.json")).expect("a session");
    let held = session.pane(9).expect("the pane");
    assert_eq!((held.held_sends, held.held_from, held.held_queued, held.draft), (2, Some(5), true, true));
    assert_eq!(held_of(Some(&session), 9).expect("a bar").text, "2 queued: the first goes right after yours");
    assert!(held_of(Some(&session), 5).is_none());
}

#[test]
fn a_view_line_without_the_fields_reads_as_no_sender_and_not_queued() {
    let session: Session = serde_json::from_str(fixture!("session/held.json")).expect("a session");
    let held = session.pane(9).expect("the pane");
    assert_eq!((held.held_from, held.held_queued), (None, false));
}

#[test]
fn nothing_held_draws_no_bar() {
    assert!(held_of(Some(&session_of(vec![holding(9, 0)])), 9).is_none());
    assert!(held_of(None, 9).is_none());
}

#[test]
fn a_pane_resuming_its_agent_has_no_bar() {
    let resuming = PaneInfo { resuming: true, ..holding(9, 1) };
    assert!(held_of(Some(&session_of(vec![resuming])), 9).is_none());
}

#[test]
fn each_button_runs_its_flag_on_the_pane() {
    assert_eq!(HeldAct::SendNow.args(9), ["held", "--pane", "9", "--send-now"]);
    assert_eq!(HeldAct::WaitTurn.args(9), ["held", "--pane", "9", "--wait-turn"]);
}

#[test]
fn the_same_pane_id_on_a_machine_is_another_job() {
    let here = HeldAct::SendNow.busy(None, 9);
    let there = HeldAct::SendNow.busy(Some("vps"), 9);
    assert_ne!(here.key(), there.key());
}

#[test]
fn the_two_buttons_of_a_pane_are_two_jobs() {
    assert_ne!(HeldAct::SendNow.busy(None, 9).key(), HeldAct::WaitTurn.busy(None, 9).key());
}

#[test]
fn only_the_send_now_of_a_person_with_a_draft_warns_of_their_line() {
    assert!(HeldAct::SendNow.tip(&bar("x", false, true)).contains("your unsent line goes with it"));
    assert!(!HeldAct::SendNow.tip(&bar("x", false, false)).contains("your unsent line"));
}

#[test]
fn a_wide_band_holds_the_words_and_both_buttons() {
    let fit = band_fit(&bar(LONG, false, false), 100).expect("a band");
    assert_eq!(fit, BandFit { text: LONG.to_string(), note: false, wait: true });
}

#[test]
fn a_draft_adds_what_send_now_does_when_it_fits() {
    let room = LONG.chars().count() + TAKES_DRAFT.chars().count() + 30;
    assert!(band_fit(&bar(LONG, false, true), room).expect("a band").note);
}

#[test]
fn a_band_short_of_room_drops_the_warning_before_it_cuts_the_words() {
    let fit = band_fit(&bar(LONG, false, true), 90).expect("a band");
    assert_eq!(fit, BandFit { text: LONG.to_string(), note: false, wait: true });
}

#[test]
fn a_narrower_band_cuts_its_words_and_keeps_both_buttons() {
    let fit = band_fit(&bar("1 message waiting", false, false), 35).expect("a band");
    assert_eq!(fit, BandFit { text: "1 message …".to_string(), note: false, wait: true });
}

#[test]
fn a_narrower_band_drops_its_words_before_wait_turn() {
    let fit = band_fit(&bar("1 message waiting", false, false), 24).expect("a band");
    assert_eq!(fit, BandFit { text: String::new(), note: false, wait: true });
}

#[test]
fn send_now_is_the_last_to_go() {
    let fit = band_fit(&bar("1 message waiting", false, false), 10).expect("a band");
    assert_eq!(fit, BandFit { text: String::new(), note: false, wait: false });
}

#[test]
fn no_room_for_send_now_draws_no_band() {
    assert!(band_fit(&bar("1 message waiting", false, false), 9).is_none());
}

#[test]
fn a_chosen_wait_turn_leaves_room_for_its_mark_and_the_spinner() {
    let plain = band_fit(&bar("1 message waiting and more", false, false), 40).expect("a band");
    let queued = band_fit(&bar("1 message waiting and more", true, false), 40).expect("a band");
    assert_eq!(plain.text.chars().count(), queued.text.chars().count() + 2 * ICON_ROOM);
}

#[test]
fn a_pane_at_the_bottom_holding_nothing_has_no_notes() {
    assert!(notes_of(&scrolled(402, 354, 48), None, false).is_none());
}

#[test]
fn a_pane_scrolled_up_counts_the_lines_below_it() {
    let notes = notes_of(&scrolled(402, 324, 48), None, false).expect("notes");
    assert_eq!(notes.below, 30);
    assert!(notes.held.is_none());
}

#[test]
fn a_pane_holding_a_message_says_so_at_the_bottom_too() {
    let notes = notes_of(&scrolled(48, 0, 48), Some(bar("2 messages", false, false)), false).expect("notes");
    assert_eq!(notes.below, 0);
    assert_eq!(notes.held, Some(bar("2 messages", false, false)));
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
