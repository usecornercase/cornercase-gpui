use serde_json::{Value, json};

use super::*;
use crate::model::surface::{Branch, CursorShape};

fn parse<T: serde::de::DeserializeOwned>(text: &str) -> T {
    serde_json::from_str(text).expect("valid JSON")
}

fn cell(pane: &PaneGrid, x: u16, y: u16) -> (&str, &Run) {
    pane.cell(x, y).expect("a cell there")
}

fn rows(texts: &[&str]) -> Value {
    Value::Array(texts.iter().map(|text| json!([{ "style": 0, "cells": [text] }])).collect())
}

fn complete(panes: &Value) -> Frame {
    serde_json::from_value(json!({
        "boot": "b", "revision": 1, "surface": 3, "tab": 1, "cols": 4, "rows": 3, "styles": [{}], "panes": panes
    }))
    .expect("a complete frame")
}

fn patch(panes: &Value) -> Frame {
    Frame { base: Some(1), revision: 2, ..complete(panes) }
}

fn texts(grid: &Grid, pane: u64) -> Vec<String> {
    let held = grid.pane(pane).expect("a pane");
    (0..held.lines.len())
        .map(|y| held.cell(0, u16::try_from(y).expect("a small row")).expect("a cell").0.to_owned())
        .collect()
}

fn held(panes: &[(u64, &[&str])]) -> Grid {
    let mut grid = Grid::default();
    let panes = panes.iter().map(|(pane, text)| json!({ "pane": pane, "lines": rows(text) }));
    assert!(matches!(grid.apply(complete(&Value::Array(panes.collect()))), Applied::Ok));
    grid
}

fn refused(grid: &mut Grid, panes: &Value) {
    assert!(matches!(grid.apply(patch(panes)), Applied::NeedRedraw));
    assert_eq!(grid.revision, 1);
}

#[test]
fn a_row_past_the_pane_is_not_held_and_asks_for_a_whole_frame() {
    let mut grid = held(&[(7, &["a", "b", "c"])]);
    let changed = json!({ "0": [{ "style": 0, "cells": ["x"] }], "3": [{ "style": 0, "cells": ["y"] }] });
    refused(&mut grid, &json!([{ "pane": 7, "lines": changed }]));
    assert_eq!(texts(&grid, 7), ["a", "b", "c"]);
}

#[test]
fn a_row_number_past_any_screen_neither_grows_the_grid_nor_panics() {
    let mut grid = held(&[(7, &["a", "b", "c"])]);
    for row in ["70000", "18446744073709551615"] {
        let changed = json!({ row: [{ "style": 0, "cells": ["y"] }] });
        refused(&mut grid, &json!([{ "pane": 7, "lines": changed }]));
        assert_eq!(grid.pane(7).expect("a pane").lines.len(), 3);
    }
}

#[test]
fn a_copy_reaching_past_the_pane_moves_nothing_and_asks_for_a_whole_frame() {
    let mut grid = held(&[(7, &["a", "b", "c"])]);
    refused(&mut grid, &json!([{ "pane": 7, "copy": { "from": 1, "to": 2, "count": 2 } }]));
    assert_eq!(texts(&grid, 7), ["a", "b", "c"]);
}

#[test]
fn a_copy_whose_source_reaches_past_the_pane_moves_nothing_and_asks_for_a_whole_frame() {
    let mut grid = held(&[(7, &["a", "b", "c"])]);
    refused(&mut grid, &json!([{ "pane": 7, "copy": { "from": 2, "to": 0, "count": 2 } }]));
    assert_eq!(texts(&grid, 7), ["a", "b", "c"]);
}

#[test]
fn more_rows_than_the_pane_held_ask_for_a_whole_frame() {
    let mut grid = held(&[(7, &["a", "b", "c"])]);
    refused(&mut grid, &json!([{ "pane": 7, "lines": rows(&["w", "x", "y", "z"]) }]));
    assert_eq!(texts(&grid, 7), ["a", "b", "c"]);
}

#[test]
fn a_patch_naming_a_pane_not_held_changes_no_other_pane() {
    let mut grid = held(&[(7, &["a"]), (8, &["b"])]);
    let changed = json!({ "0": [{ "style": 0, "cells": ["x"] }] });
    refused(&mut grid, &json!([{ "pane": 7, "lines": changed }, { "pane": 99 }]));
    assert_eq!(texts(&grid, 7), ["a"]);
}

#[test]
fn a_copy_down_over_its_own_rows_moves_the_old_ones() {
    let mut grid = held(&[(7, &["a", "b", "c"])]);
    assert!(matches!(
        grid.apply(patch(&json!([{ "pane": 7, "copy": { "from": 0, "to": 1, "count": 2 } }]))),
        Applied::Ok
    ));
    assert_eq!(texts(&grid, 7), ["a", "a", "b"]);
}

#[test]
fn a_changed_row_past_the_rows_held_is_dropped_whatever_called_for_it() {
    let mut grid = held(&[(7, &["a", "b"])]);
    let tables = Tables::new(vec![Style::default()], Vec::new());
    let run = |text: &str| vec![WireRun { style: 0, hyperlink: None, cells: vec![text.to_owned()] }];
    let changed = HashMap::from([("1".to_owned(), run("x")), ("5".to_owned(), run("y"))]);
    grid.panes[0].replace_rows(changed, &tables);
    assert_eq!(texts(&grid, 7), ["a", "x"]);
}

#[test]
fn a_pane_scrolled_up_counts_the_lines_below_its_view() {
    let frame: Frame = parse(
        r#"{"surface":1,"revision":1,"tab":1,"cols":80,"rows":24,"panes":[
        {"pane":1,"scroll":{"total":402,"offset":324,"rows":48}},
        {"pane":2,"scroll":{"total":402,"offset":354,"rows":48}}]}"#,
    );
    let mut grid = Grid::default();
    assert!(matches!(grid.apply(frame), Applied::Ok));
    assert_eq!(grid.pane(1).expect("a pane").below(), 30);
    assert_eq!(grid.pane(2).expect("a pane").below(), 0);
}

#[test]
fn the_grid_keeps_the_projection_of_its_last_frame() {
    let mut frame: Value = parse(crate::test_util::fixture!("frame.json"));
    frame["projection"] = 4.into();
    let mut grid = Grid::default();
    assert!(matches!(grid.apply(serde_json::from_value(frame).expect("a frame")), Applied::Ok));
    assert_eq!(grid.projection, Some(4));
}

fn fixture_grid() -> Grid {
    let frame: Frame = parse(crate::test_util::fixture!("frame.json"));
    let mut grid = Grid::default();
    assert!(matches!(grid.apply(frame), Applied::Ok));
    grid
}

#[test]
fn reads_the_fixture_frame() {
    let grid = fixture_grid();
    assert_eq!(grid.panes.len(), 2);
    let pane = &grid.panes[0];
    assert_eq!(cell(pane, 1, 1).0, "k");
    assert!(cell(pane, 1, 1).1.style.bold);
    assert_eq!(cell(pane, 3, 1).0, "中");
    assert_eq!(cell(pane, 2, 2).1.link.as_deref(), None);
    assert_eq!(cell(pane, 3, 2).1.link.as_deref(), Some("https://example.com/docs/a-long-page"));
    assert_eq!(pane.cursor.map(|c| c.shape), Some(CursorShape::Bar));
    assert_eq!(pane.colours.palette.get("1"), Some(&[255, 0, 0]));
}

#[test]
fn keeps_the_split_tree_of_the_fixture_frame() {
    let split = |first, second| Node::Split(Box::new(Branch { dir: "right".into(), first, second }));
    assert_eq!(fixture_grid().layout, Some(split(Node::Leaf(13), Node::Leaf(14))));
}

#[test]
fn applies_a_patch_with_a_copy() {
    let full = r#"{"boot":"b","revision":1,"surface":3,"tab":1,"cols":4,"rows":3,"styles":[{}],
        "panes":[{"pane":7,"active":true,"rect":{"x":0,"y":0,"cols":4,"rows":3},"size":{"cols":4,"rows":3},
        "cursor":null,"lines":[[{"style":0,"cells":["a"]}],[{"style":0,"cells":["b"]}],[{"style":0,"cells":["c"]}]]}]}"#;
    let patch = r#"{"boot":"b","revision":2,"base":1,"surface":3,"tab":1,"cols":4,"rows":3,
        "styles":[{"bold":true}],"panes":[{"pane":7,"cursor":{"x":1,"y":2},
        "copy":{"from":1,"to":0,"count":2},"lines":{"2":[{"style":0,"cells":["d"]}]}}]}"#;
    let mut grid = Grid::default();
    grid.apply(parse(full));
    assert!(matches!(grid.apply(parse(patch)), Applied::Ok));
    let pane = &grid.panes[0];
    let text: Vec<&str> = (0..3).map(|y| cell(pane, 0, y).0).collect();
    assert_eq!(text, ["b", "c", "d"]);
    assert!(cell(pane, 0, 2).1.style.bold);
    assert_eq!(pane.cursor.map(|c| c.x), Some(1));
    assert_eq!(grid.revision, 2);
    let stale = patch.replace("\"revision\":2,\"base\":1", "\"revision\":9,\"base\":5");
    assert!(matches!(grid.apply(parse(&stale)), Applied::NeedRedraw));
}

fn popup_grid() -> Grid {
    let mut grid = Grid::default();
    assert!(matches!(grid.apply(parse(crate::test_util::fixture!("surface/popup.json"))), Applied::Ok));
    grid
}

#[test]
fn a_popup_frame_names_its_plugin_and_holds_one_pane() {
    let grid = popup_grid();
    let popup = grid.popup.as_ref().expect("a popup");
    assert_eq!((popup.pane, popup.plugin.as_str(), popup.action.as_str()), (40, "acme.notes", "browse"));
    assert_eq!((grid.tab, grid.cols, grid.rows, grid.layout.is_none()), (0, 60, 10, true));
    assert_eq!(grid.panes.iter().map(|p| p.pane).collect::<Vec<_>>(), [40]);
    assert!(cell(&grid.panes[0], 0, 1).1.style.bold);
}

#[test]
fn a_popup_patch_keeps_the_popup_and_changes_its_rows() {
    let mut grid = popup_grid();
    assert!(matches!(grid.apply(parse(crate::test_util::fixture!("surface/popup-patch.json"))), Applied::Ok));
    assert_eq!(grid.popup.as_ref().map(|p| p.pane), Some(40));
    assert_eq!(cell(&grid.panes[0], 2, 0).0, "n");
    assert_eq!(grid.panes[0].cursor.map(|c| c.x), Some(3));
}

#[test]
fn a_tab_frame_has_no_popup() {
    assert!(fixture_grid().popup.is_none());
}
