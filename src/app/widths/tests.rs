use super::{dock_room, of, restored};
use crate::app::keeper::Widths;

fn saved(sidebar: Option<f32>, panel: Option<f32>) -> Widths {
    Widths { sidebar, panel, ..Widths::default() }
}

fn size_of(chosen: Option<f32>) -> f32 {
    chosen.expect("a width was chosen")
}

#[test]
fn widths_that_fit_come_back_as_saved() {
    let (sidebar, _, docks) = restored(&saved(Some(300.), Some(500.)), 1440.);
    assert!((sidebar.size() - 300.).abs() < f32::EPSILON);
    assert!((docks.panel.size() - 500.).abs() < f32::EPSILON);
}

#[test]
fn a_restored_sidebar_leaves_the_terminal_and_a_dock_their_room() {
    let (sidebar, ..) = restored(&saved(Some(560.), None), 900.);
    assert!((sidebar.size() - 284.).abs() < f32::EPSILON);
}

#[test]
fn a_restored_dock_keeps_the_saved_width_for_the_window_to_fit() {
    let (_, _, docks) = restored(&saved(Some(300.), Some(1208.)), 1280.);
    assert!((docks.panel.size() - 1208.).abs() < f32::EPSILON);
}

#[test]
fn the_dock_room_follows_the_sidebar_shown() {
    assert!((dock_room(1280., 300.) - 672.).abs() < f32::EPSILON);
    assert!((dock_room(1280., 700.) - 272.).abs() < f32::EPSILON);
}

#[test]
fn a_restored_width_keeps_its_split_limits() {
    let (sidebar, ..) = restored(&saved(Some(10.), None), 1440.);
    assert!((sidebar.size() - 200.).abs() < f32::EPSILON);
    let (_, _, docks) = restored(&saved(None, Some(5000.)), 4000.);
    assert!((docks.panel.size() - 1208.).abs() < f32::EPSILON);
}

#[test]
fn widths_never_dragged_stay_unset() {
    let (sidebar, pair, docks) = restored(&Widths::default(), 1440.);
    assert_eq!(of(sidebar, pair, docks), Widths::default());
}

#[test]
fn the_widths_read_back_what_the_splits_chose() {
    let (sidebar, pair, docks) = restored(&saved(Some(300.), Some(500.)), 1440.);
    let back = of(sidebar, pair, docks);
    assert_eq!(size_of(back.sidebar), 300.);
    assert_eq!(size_of(back.panel), 500.);
    assert_eq!(back.pair, None);
}
