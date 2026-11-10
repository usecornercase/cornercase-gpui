use serde_json::{Value, json};

use super::*;

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
    serde_json::from_value(json!({
        "boot": "b", "revision": 2, "base": 1, "surface": 3, "tab": 1, "cols": 4, "rows": 3,
        "styles": [{}], "panes": panes
    }))
    .expect("a patch")
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
fn a_patch_within_its_pane_keeps_the_rows_it_had() {
    let mut grid = held(&[(7, &["a", "b", "c"])]);
    let changed = json!({ "2": [{ "style": 0, "cells": ["d"] }] });
    let moved = json!([{ "pane": 7, "copy": { "from": 1, "to": 0, "count": 2 }, "lines": changed }]);
    assert!(matches!(grid.apply(patch(&moved)), Applied::Ok));
    assert_eq!(texts(&grid, 7), ["b", "c", "d"]);
    assert_eq!(grid.revision, 2);
}
