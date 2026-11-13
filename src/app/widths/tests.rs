use super::{Arranged, arranged, dock_room, of, restored};
use crate::app::keeper::Widths;
use crate::app::{DOCK, PAIR, SIDEBAR, TERMINAL_KEEP, WIDE_DOCK, wide_default};
use crate::sidebar::rail::COLUMN;
use crate::ui::split::Split;

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

#[expect(clippy::unnecessary_wraps, reason = "the arrangement takes an option")]
fn plain(wanted: f32) -> Option<(Split, f32)> {
    Some((DOCK, wanted))
}

#[expect(clippy::unnecessary_wraps, reason = "the arrangement takes an option")]
fn wide(window: f32) -> Option<(Split, f32)> {
    Some((WIDE_DOCK, wide_default(window)))
}

fn terminal_left(window: f32, sidebar: f32, now: Arranged) -> f32 {
    window - COLUMN - if now.sidebar { sidebar + 8. } else { 0. } - now.dock
}

#[test]
fn at_the_least_width_a_plain_panel_takes_the_sidebars_place() {
    let now = arranged(640., 256., plain(428.));
    assert_eq!(now, Arranged { sidebar: false, dock: 340. });
    assert!((terminal_left(640., 256., now) - 240.).abs() < f32::EPSILON);
}

#[test]
fn at_the_least_width_a_browser_or_flow_panel_keeps_its_least_and_the_terminal_gives_way() {
    let now = arranged(640., 256., wide(640.));
    assert_eq!(now, Arranged { sidebar: false, dock: 420. });
    assert!((terminal_left(640., 256., now) - 160.).abs() < f32::EPSILON);
}

#[test]
fn where_everything_fits_the_layout_is_as_before() {
    assert_eq!(arranged(1000., 256., plain(428.)), Arranged { sidebar: true, dock: 428. });
    assert_eq!(arranged(1000., 256., wide(1000.)), Arranged { sidebar: true, dock: 436. });
    assert_eq!(arranged(1280., 256., plain(428.)), Arranged { sidebar: true, dock: 428. });
    assert_eq!(arranged(1280., 256., wide(1280.)), Arranged { sidebar: true, dock: 640. });
}

#[test]
fn the_dock_shrinks_to_its_least_before_the_sidebar_steps_aside() {
    let least = 256. + COLUMN + 8. + TERMINAL_KEEP + DOCK.min();
    assert_eq!(arranged(least, 256., plain(428.)), Arranged { sidebar: true, dock: DOCK.min() });
    assert!(!arranged(least - 1., 256., plain(428.)).sidebar);
}

fn assert_fits(window: f32, sidebar: f32, split: Split, wanted: f32) {
    let now = arranged(window, sidebar, Some((split, wanted)));
    assert_eq!(now.sidebar, dock_room(window, sidebar) >= split.min(), "{window} {sidebar}");
    assert!(now.dock >= split.min());
    let terminal = terminal_left(window, sidebar, now);
    assert!(terminal >= 160., "{window} {sidebar} {now:?}");
    assert!(terminal >= TERMINAL_KEEP || now.dock <= split.min(), "{window} {sidebar} {now:?}");
}

#[test]
fn nothing_runs_past_the_window_from_its_least_width_up() {
    let docks = [(DOCK, 428.), (DOCK, 1208.), (WIDE_DOCK, 0.), (WIDE_DOCK, 420.), (WIDE_DOCK, 4096.)];
    for window in (80..=320u16).map(|step| f32::from(step) * 8.) {
        for (sidebar, (split, chosen)) in [200., 256., 456., 560.].into_iter().flat_map(|s| docks.map(|d| (s, d))) {
            let wanted = if chosen == 0. { wide_default(window) } else { chosen };
            assert_fits(window, sidebar, split, wanted);
        }
    }
}

#[test]
fn a_dragged_dock_width_comes_back_when_the_window_widens() {
    let dragged = DOCK.restored(Some(600.), f32::MAX);
    assert!((arranged(640., 256., Some((dragged, dragged.size()))).dock - 340.).abs() < f32::EPSILON);
    assert!((arranged(1440., 256., Some((dragged, dragged.size()))).dock - 600.).abs() < f32::EPSILON);
    assert_eq!(dragged.chosen(), Some(600.));
}

#[test]
fn a_layout_saved_wide_and_read_into_the_least_width_keeps_the_rule() {
    let wide_layout =
        Widths { sidebar: Some(300.), panel: Some(600.), browser: Some(900.), flow: Some(800.), ..Widths::default() };
    let (sidebar, _, docks) = restored(&wide_layout, 640.);
    for (split, size) in [(docks.panel, 600.), (docks.browser, 900.), (docks.flow, 800.)] {
        let now = arranged(640., sidebar.size(), Some((split, split.size())));
        assert!(!now.sidebar);
        assert!(terminal_left(640., sidebar.size(), now) >= 160., "{size}");
    }
    let (sidebar, _, docks) = restored(&wide_layout, 1440.);
    let now = arranged(1440., sidebar.size(), Some((docks.browser, docks.browser.size())));
    assert_eq!(now, Arranged { sidebar: true, dock: 832. });
}

#[test]
fn without_a_docked_panel_the_sidebar_stays() {
    assert!(arranged(640., 456., None).sidebar);
    assert!(arranged(640., PAIR.size(), None).sidebar);
    assert!(arranged(640., SIDEBAR.size(), None).dock.abs() < f32::EPSILON);
}
