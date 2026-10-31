use super::*;
use crate::model::{Run, Style};
use crate::test_util::fixture;

const INSIDE: &str = fixture!("link/inside.json");

const OUTSIDE: &str = r#"{"pane":4,"link":{"path":"/home/me/plan.md","inside":false,"workspace":2,
    "row":1,"start":0,"end":9}}"#;

fn link(json: &str) -> Link {
    Link::parse(json).expect("a link")
}

fn pane_showing(rows: &[&[&str]]) -> PaneGrid {
    let lines = rows
        .iter()
        .map(|cells| {
            let cells = cells.iter().map(|c| (*c).to_string()).collect();
            vec![Run { style: Style::default(), link: None, cells }]
        })
        .collect();
    PaneGrid { lines, ..PaneGrid::default() }
}

#[test]
fn reads_what_a_click_on_a_cell_opens() {
    let link = link(INSIDE);
    assert_eq!((link.path.as_str(), link.inside, link.workspace), ("/Users/me/shop/src/main.rs", true, Some(36)));
    assert_eq!((link.row, link.start, link.end), (0, 0, 19));
    assert_eq!(link.lines, Some(Lines { first: 120, last: 130 }));
}

#[test]
fn a_cell_on_no_file_reads_as_none() {
    assert_eq!(Link::parse(fixture!("link/none.json")), None);
    assert_eq!(Link::parse("not json"), None);
}

#[test]
fn an_empty_span_reads_as_none() {
    assert_eq!(Link::parse(r#"{"pane":4,"link":{"path":"/a","row":0,"start":3,"end":3}}"#), None);
}

#[test]
fn the_span_covers_its_cells_and_not_the_one_after() {
    let link = link(INSIDE);
    assert!(link.covers((0, 0)) && link.covers((18, 0)));
    assert!(!link.covers((19, 0)) && !link.covers((10, 1)));
}

#[test]
fn a_file_inside_the_workspace_shows_its_relative_path() {
    assert_eq!(link(INSIDE).shown_path(), "src/main.rs");
    assert_eq!(link(OUTSIDE).shown_path(), "/home/me/plan.md");
}

#[test]
fn lines_are_counted_from_zero() {
    let inside = link(INSIDE);
    assert_eq!((inside.first_line(), inside.last_line()), (Some(119), Some(129)));
    assert_eq!(link(OUTSIDE).first_line(), None);
}

#[test]
fn the_name_is_the_last_part_of_the_path() {
    assert_eq!(link(OUTSIDE).name(), "plan.md");
}

#[test]
fn the_same_file_at_other_lines_is_another_peek() {
    let (a, mut b) = (link(INSIDE), link(INSIDE));
    b.start = 0;
    assert!(a.same_file(&b));
    b.lines = None;
    assert!(!a.same_file(&b));
}

#[test]
fn a_row_reads_as_its_cells_with_wide_characters_whole() {
    let pane = pane_showing(&[&["a", "界", "", "b"]]);
    assert_eq!(text_between(&row_cells(&pane, 0), 0, 4), "a界b");
    assert_eq!(row_cells(&pane, 3), Vec::<&str>::new());
}

#[test]
fn a_span_still_shows_its_text_until_the_row_changes() {
    let pane = pane_showing(&[&["x", " ", "a", ".", "r", "s"]]);
    assert!(shows(&pane, (0, 2, 6), "a.rs"));
    assert!(!shows(&pane, (0, 2, 6), "b.rs"));
    assert!(!shows(&pane, (1, 2, 6), "a.rs"));
}
