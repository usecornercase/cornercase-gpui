use gpui::{Bounds, Pixels, point, px, size};

use super::layout::Layout;
use crate::model::{Area, Branch, Grid, Node, PaneGrid};

pub(super) const CELL: (f32, f32) = (8., 16.);

pub(super) fn pane(pane: u64, (x, y): (u16, u16), (cols, rows): (u16, u16)) -> PaneGrid {
    PaneGrid { pane, rect: Area { x, y, cols, rows }, ..Default::default() }
}

pub(super) fn split(dir: &str, first: Node, second: Node) -> Node {
    Node::Split(Box::new(Branch { dir: dir.into(), first, second }))
}

pub(super) fn area(w: f32, h: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(0.), px(0.)), size(px(w), px(h)))
}

pub(super) fn side_by_side() -> Grid {
    Grid {
        cols: 40,
        rows: 10,
        layout: Some(split("right", Node::Leaf(1), Node::Leaf(2))),
        panes: vec![pane(1, (0, 0), (19, 10)), pane(2, (21, 0), (19, 10))],
        ..Default::default()
    }
}

pub(super) fn beside_a_stack() -> Grid {
    Grid {
        cols: 40,
        rows: 21,
        layout: Some(split("right", Node::Leaf(1), split("down", Node::Leaf(2), Node::Leaf(3)))),
        panes: vec![pane(1, (0, 0), (19, 21)), pane(2, (21, 0), (19, 10)), pane(3, (21, 11), (19, 10))],
        ..Default::default()
    }
}

pub(super) fn bounds(layout: &Layout, pane: u64) -> Bounds<Pixels> {
    layout.card(pane).expect("a card for the pane").bounds
}
