use gpui::{Modifiers, MouseButton, MouseDownEvent, point, px};

use super::*;
use crate::model::{Area, Run, Style};

fn pane_showing(text: &str) -> PaneGrid {
    let cells = text.chars().map(|c| c.to_string()).collect();
    let mut pane = PaneGrid {
        pane: 7,
        rect: Area { x: 4, y: 2, cols: 40, rows: 3 },
        lines: vec![vec![Run { style: Style::default(), link: None, cells }]],
        ..PaneGrid::default()
    };
    (pane.size.cols, pane.size.rows) = (40, 3);
    pane
}

fn press(button: MouseButton, click_count: usize, modifiers: Modifiers) -> MouseDownEvent {
    MouseDownEvent { button, position: point(px(0.), px(0.)), modifiers, click_count, first_mouse: false }
}

#[test]
fn the_word_under_the_pointer_is_the_whole_path() {
    let pane = pane_showing("see src/main.rs:12 now");
    let spot = spot_at(&pane, (6, 0)).expect("a word");
    assert_eq!((spot.pane, spot.row, spot.start, spot.end), (7, 0, 4, 18));
    assert_eq!(spot.text, "src/main.rs:12");
}

#[test]
fn a_blank_or_a_bracket_is_no_word() {
    let pane = pane_showing("(a.rs)  x");
    assert_eq!(spot_at(&pane, (0, 0)), None);
    assert_eq!(spot_at(&pane, (7, 0)), None);
    assert_eq!(spot_at(&pane, (39, 0)), None);
    assert_eq!(spot_at(&pane, (2, 0)).map(|s| s.text), Some("a.rs".to_string()));
}

#[test]
fn a_cell_of_the_pane_counts_from_its_corner() {
    let pane = pane_showing("");
    assert_eq!(inside(&pane, (4, 2)), Some((0, 0)));
    assert_eq!(inside(&pane, (43, 4)), Some((39, 2)));
}

#[test]
fn the_padding_around_a_pane_is_no_cell() {
    let pane = pane_showing("");
    assert_eq!(inside(&pane, (3, 2)), None);
    assert_eq!(inside(&pane, (44, 2)), None);
    assert_eq!(inside(&pane, (4, 5)), None);
}

#[test]
fn asking_for_a_cell_names_the_pane_and_the_cell() {
    assert_eq!(link_args(7, (3, 1)), ["link", "--pane", "7", "--cell", "3,1", "--json"]);
}

#[test]
fn only_a_plain_left_click_opens_a_link() {
    let shift = Modifiers { shift: true, ..Modifiers::default() };
    assert!(plain_click(&press(MouseButton::Left, 1, Modifiers::default())));
    assert!(!plain_click(&press(MouseButton::Left, 2, Modifiers::default())));
    assert!(!plain_click(&press(MouseButton::Left, 1, shift)));
    assert!(!plain_click(&press(MouseButton::Right, 1, Modifiers::default())));
}
