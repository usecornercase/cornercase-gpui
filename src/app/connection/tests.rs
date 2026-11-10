use serde_json::json;

use super::*;
use crate::cli::QueueTap;
use crate::model::ViewInfo;

fn tapped() -> (Connection, QueueTap) {
    let (queue, tap) = QueueTap::new();
    (Connection::new(Target::Local, queue), tap)
}

fn line(view: u64, size: Option<(u16, u16)>) -> Session {
    let info = ViewInfo { id: view, cols: size.map(|s| s.0), rows: size.map(|s| s.1), ..ViewInfo::default() };
    Session { view: Some(info), ..Session::default() }
}

fn with_rows(surface: u64, revision: u64, base: Option<u64>, panes: &serde_json::Value) -> Frame {
    serde_json::from_value(json!({
        "boot": "b", "surface": surface, "revision": revision, "base": base, "tab": 6, "styles": [{}], "panes": panes
    }))
    .expect("a frame with rows")
}

fn complete(surface: u64, revision: u64) -> Frame {
    with_rows(surface, revision, None, &json!([]))
}

fn patch_on(base: u64) -> Frame {
    Frame { base: Some(base), ..complete(4, base + 1) }
}

#[test]
fn a_connection_owns_what_its_own_streams_hear() {
    let conn = Connection::with_serials(7, 8);
    assert!(conn.owns(7));
    assert!(conn.owns(8));
    assert!(!conn.owns(9));
}

#[test]
fn a_connection_with_no_stream_hears_no_serial() {
    assert!(!Connection::unopened(None).owns(0));
}

#[test]
fn a_closed_connection_hears_nothing_more_from_its_streams() {
    let mut conn = Connection::with_serials(7, 8);
    conn.close();
    assert!(!conn.owns(7));
    assert!(!conn.owns(8));
}

#[test]
fn a_paused_surface_hears_nothing_more_while_its_view_goes_on() {
    let mut conn = Connection::with_serials(7, 8);
    conn.pause_surface();
    assert!(conn.owns(7));
    assert!(!conn.owns(8));
}

#[test]
fn two_streams_never_share_a_serial() {
    assert_ne!(next_serial(), next_serial());
}

#[test]
fn a_view_line_names_the_view_once() {
    let mut conn = Connection::unopened(None);
    assert!(conn.saw_view(&line(9, None)));
    assert!(!conn.saw_view(&line(9, None)));
    assert_eq!(conn.view_id, Some(9));
}

#[test]
fn a_new_view_is_told_the_size_wanted_by_now() {
    let (mut conn, tap) = tapped();
    conn.size = Some((100, 30));
    conn.saw_view(&line(9, Some((80, 24))));
    assert_eq!(tap.commands(), [["view-set", "--view", "9", "--cols", "100", "--rows", "30"]]);
}

#[test]
fn a_view_already_at_the_size_wanted_is_told_nothing() {
    let (mut conn, tap) = tapped();
    conn.size = Some((80, 24));
    conn.saw_view(&line(9, Some((80, 24))));
    assert_eq!(tap.commands(), Vec::<Vec<String>>::new());
}

#[test]
fn a_surface_waits_for_a_tab() {
    let mut conn = Connection::unopened(None);
    conn.saw_view(&line(9, None));
    assert_eq!(conn.surface_due(false), None);
    assert_eq!(conn.surface_due(true), Some(9));
}

#[test]
fn a_connection_acts_with_its_own_view() {
    let (mut conn, tap) = tapped();
    conn.view_id = Some(9);
    conn.act(vec!["focus".into(), "--pane".into(), "5".into()]);
    assert_eq!(tap.commands(), [["focus", "--pane", "5", "--view", "9"]]);
}

#[test]
fn a_connection_with_no_view_acts_on_nothing() {
    let (conn, tap) = tapped();
    conn.act(vec!["focus".into(), "--pane".into(), "5".into()]);
    conn.send_size((80, 24));
    conn.tell_visible(false);
    assert_eq!(tap.commands(), Vec::<Vec<String>>::new());
}

#[test]
fn a_connection_tells_its_view_when_the_window_hides() {
    let (mut conn, tap) = tapped();
    conn.view_id = Some(9);
    conn.tell_visible(false);
    conn.tell_visible(true);
    assert_eq!(tap.commands(), [["view-set", "--view", "9", "--hidden"], ["view-set", "--view", "9", "--visible"]]);
}

#[test]
fn a_complete_frame_shows() {
    let mut conn = Connection::unopened(None);
    conn.surface_error = Some("why".into());
    assert!(conn.frame_arrived(complete(4, 1)));
    assert!(conn.has_frame);
    assert_eq!(conn.surface_error, None);
}

#[test]
fn a_patch_without_its_base_asks_for_a_whole_frame_on_its_own_queue() {
    let (mut conn, tap) = tapped();
    conn.frame_arrived(complete(4, 1));
    assert!(!conn.frame_arrived(patch_on(5)));
    assert_eq!(tap.commands(), [["redraw", "--surface", "4"]]);
}

#[test]
fn a_connection_gone_nowhere_sends_nothing_and_keeps_its_last_frame() {
    let (mut conn, tap) = tapped();
    conn.view_id = Some(9);
    conn.frame_arrived(complete(4, 1));
    conn.go_nowhere();
    conn.queue.input(5, json!({ "text": "a" }));
    conn.queue.cmd(vec!["focus".into()]);
    assert!(conn.has_frame);
    assert_eq!(tap.inputs(), Vec::new());
    assert_eq!(tap.commands(), Vec::<Vec<String>>::new());
}

#[test]
fn a_patch_past_the_pane_asks_for_a_whole_frame_on_its_own_queue() {
    let (mut conn, tap) = tapped();
    let two = json!([{ "pane": 9, "lines": [[{ "style": 0, "cells": ["a"] }], [{ "style": 0, "cells": ["b"] }]] }]);
    conn.frame_arrived(with_rows(4, 1, None, &two));
    let past = json!([{ "pane": 9, "lines": { "2": [{ "style": 0, "cells": ["c"] }] } }]);
    assert!(!conn.frame_arrived(with_rows(4, 2, Some(1), &past)));
    assert_eq!(tap.commands(), [["redraw", "--surface", "4"]]);
    assert_eq!(conn.grid.panes[0].lines.len(), 2);
}
