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

#[test]
fn a_web_address_under_the_pointer_is_found_without_cornercase() {
    let pane = pane_showing("open http://localhost:3000/. now");
    assert_eq!(web_at(&pane, (12, 0)).as_deref(), Some("http://localhost:3000/"));
    assert_eq!(web_at(&pane, (27, 0)), None);
    assert_eq!(web_at(&pane, (1, 0)), None);
}

#[test]
fn the_part_of_a_word_before_its_address_opens_nothing() {
    let pane = pane_showing("url=https://a.dev");
    let spot = spot_at(&pane, (1, 0)).expect("a word");
    let (span, url) = web_under(&pane, &spot).expect("an address");
    assert_eq!((span.start, span.end, url.as_str()), (4, 17, "https://a.dev"));
    assert_eq!(web_at(&pane, (1, 0)), None);
    assert_eq!(web_at(&pane, (4, 0)).as_deref(), Some("https://a.dev"));
}

#[test]
fn a_path_is_left_to_cornercase() {
    let pane = pane_showing("see src/main.rs:12 now");
    let spot = spot_at(&pane, (6, 0)).expect("a word");
    assert_eq!(web_under(&pane, &spot), None);
}

fn run(text: &str, link: Option<&str>) -> Run {
    Run { style: Style::default(), link: link.map(Into::into), cells: text.chars().map(String::from).collect() }
}

fn hyperlinked_pane(url: &str) -> PaneGrid {
    let mut pane = pane_showing("");
    pane.lines = vec![vec![run("see ", None), run("[the docs]", Some(url)), run(" now", None)]];
    pane
}

#[test]
fn a_hyperlink_is_one_spot_blanks_and_brackets_included() {
    let pane = hyperlinked_pane("https://a.dev/docs");
    let spot = spot_at(&pane, (8, 0)).expect("the hyperlink");
    assert_eq!((spot.start, spot.end, spot.text.as_str()), (4, 14, "[the docs]"));
    assert_eq!(web_at(&pane, (4, 0)).as_deref(), Some("https://a.dev/docs"));
}

#[test]
fn a_hyperlink_redrawn_with_another_destination_is_another_spot() {
    let (old, new) = (hyperlinked_pane("https://example.com/a"), hyperlinked_pane("https://example.com/b"));
    let (old, new) = (spot_at(&old, (5, 0)).expect("a spot"), spot_at(&new, (5, 0)).expect("a spot"));
    assert_eq!((old.text.as_str(), old.start, old.end), (new.text.as_str(), new.start, new.end));
    assert_ne!(old, new);
}
