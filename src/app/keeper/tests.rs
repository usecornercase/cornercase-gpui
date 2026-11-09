use std::path::{Path, PathBuf};

use super::{Keeper, Saved, Widths, path_in};
use crate::app::placement::{Placed, Rect, State};

fn widths(sidebar: Option<f32>, panel: Option<f32>) -> Widths {
    Widths { sidebar, panel, ..Widths::default() }
}

fn placed(x: f32) -> Placed {
    Placed { display: Some("A".into()), bounds: Rect { x, y: 64., width: 1440., height: 900. }, state: State::Windowed }
}

fn keeper(saved: Saved) -> Keeper {
    Keeper::new(PathBuf::from("/x/layout.json"), saved)
}

#[test]
fn the_layout_lives_beside_the_settings() {
    let folder = crate::prefs::folder_of(Path::new("/x/cornercase-gpui/settings.json"));
    assert_eq!(path_in(&folder), PathBuf::from("/x/cornercase-gpui/layout.json"));
}

#[test]
fn a_missing_or_broken_file_keeps_nothing() {
    assert_eq!(Saved::parse(""), Saved::default());
    assert_eq!(Saved::parse("{not json"), Saved::default());
}

#[test]
fn the_file_reads_back_what_it_wrote() {
    let saved = Saved {
        widths: Widths {
            sidebar: Some(300.),
            pair: Some(500.),
            panel: Some(520.),
            browser: Some(900.),
            flow: Some(800.),
        },
        window: Some(Placed { state: State::FullScreen, ..placed(120.) }),
    };
    assert_eq!(Saved::parse(&saved.text()), saved);
}

#[test]
fn widths_never_dragged_stay_out_of_the_file() {
    let text = Saved { widths: widths(Some(300.), None), window: None }.text();
    assert!(text.contains("sidebar"));
    assert!(!text.contains("pair"));
    assert!(!text.contains("window"));
}

#[test]
fn a_state_this_version_does_not_know_reads_as_windowed() {
    let text = r#"{"window":{"bounds":{"x":1.0,"y":2.0,"width":3.0,"height":4.0},"state":"tiled_left"}}"#;
    assert_eq!(Saved::parse(text).window.map(|w| w.state), Some(State::Windowed));
}

#[test]
fn a_drag_writes_only_the_width_it_moved() {
    let mut keeper = keeper(Saved { widths: widths(Some(560.), Some(500.)), window: None });
    keeper.start(widths(Some(400.), Some(500.)));
    assert!(keeper.take_widths(widths(Some(400.), Some(600.))));
    assert_eq!(keeper.kept_widths(), widths(Some(560.), Some(600.)));
}

#[test]
fn a_double_click_forgets_a_width() {
    let kept = widths(Some(300.), Some(500.));
    assert_eq!(kept.follow(kept, widths(Some(300.), None)), widths(Some(300.), None));
}

#[test]
fn a_launch_that_changes_nothing_writes_nothing() {
    let mut keeper = keeper(Saved { widths: widths(Some(560.), None), window: None });
    keeper.start(widths(Some(400.), None));
    assert_eq!(keeper.pending(), None);
    assert!(!keeper.take_widths(widths(Some(400.), None)));
}

#[test]
fn a_moved_window_is_written() {
    let mut keeper = keeper(Saved { widths: Widths::default(), window: Some(placed(10.)) });
    keeper.take_window(placed(333.));
    assert!(keeper.pending().is_some_and(|text| text.contains("333")));
}

fn zoomed(x: f32) -> Placed {
    Placed { bounds: Rect { x, y: 0., width: 1500., height: 900. }, state: State::Maximised, ..placed(x) }
}

#[test]
fn a_zoom_keeps_the_windowed_place_whatever_its_steps_were() {
    let mut keeper = keeper(Saved { widths: Widths::default(), window: Some(placed(240.)) });
    keeper.latest = Some(placed(1000.));
    keeper.apply_window(Some(zoomed(1.)));
    assert_eq!(keeper.kept.window, Some(Placed { state: State::Maximised, ..placed(240.) }));
}

#[test]
fn a_window_that_could_not_be_read_again_uses_the_last_event() {
    let mut keeper = keeper(Saved::default());
    keeper.latest = Some(placed(77.));
    keeper.apply_window(None);
    assert!(keeper.pending().is_some_and(|text| text.contains("77")));
}

#[test]
fn a_first_change_while_maximised_keeps_the_size_it_opened_at() {
    let mut keeper = keeper(Saved::default());
    keeper.opened_at(placed(116.));
    keeper.take_window(zoomed(0.));
    assert_eq!(keeper.kept.window, Some(Placed { state: State::Maximised, ..placed(116.) }));
}

#[test]
fn a_window_still_where_it_opened_writes_nothing() {
    let mut keeper = keeper(Saved::default());
    keeper.opened_at(placed(116.));
    keeper.take_window(placed(116.));
    assert_eq!(keeper.pending(), None);
}

#[test]
fn a_change_reaches_the_file_and_a_second_write_has_nothing_to_add() {
    let folder = std::env::temp_dir().join(format!("cornercase-gpui-keeper-{}", std::process::id()));
    let path = path_in(&folder);
    let mut keeper = Keeper::new(path.clone(), Saved::default());
    keeper.start(Widths::default());
    keeper.take_widths(widths(Some(321.), None));
    keeper.take_window(placed(55.));
    keeper.write().expect("the layout is written");
    let on_disk = Saved::parse(&std::fs::read_to_string(&path).expect("the file exists"));
    assert_eq!(on_disk, keeper.kept);
    assert_eq!(keeper.pending(), None);
    std::fs::remove_dir_all(&folder).expect("the folder is removed");
}
