use gpui::{point, px};

use super::*;
use crate::model::{Grid, Node};
use crate::render::fixtures::{CELL, area, beside_a_stack, bounds, pane, side_by_side};

#[test]
fn a_single_pane_fills_the_area_with_its_cells_inside_the_padding() {
    let grid = Grid { layout: Some(Node::Leaf(7)), panes: vec![pane(7, (0, 0), (10, 4))], ..Default::default() };
    let layout = Layout::new(&grid, CELL, area(200., 100.));
    assert_eq!((bounds(&layout, 7), layout.cards[0].cells), (area(200., 100.), point(px(PADDING), px(PADDING))));
}

#[test]
fn panes_side_by_side_float_a_gap_apart() {
    let layout = Layout::new(&side_by_side(), CELL, area(400., 200.));
    let (left, right) = (bounds(&layout, 1), bounds(&layout, 2));
    assert_eq!(f32::from(left.size.width), 19. * CELL.0 + PADDING * 2.);
    assert_eq!(f32::from(right.origin.x - left.right()), GAP);
    assert_eq!(right.right(), px(400.));
}

#[test]
fn the_size_asked_for_leaves_room_for_the_padding_and_the_gaps() {
    let gutter = 2. * CELL.0;
    assert_eq!(Layout::extra(&side_by_side(), CELL), (PADDING * 4. + GAP - gutter, PADDING * 2.));
}

#[test]
fn a_press_in_the_gap_takes_the_divider() {
    let layout = Layout::new(&side_by_side(), CELL, area(400., 200.));
    let hit = layout.hit(point(px(180.), px(50.)));
    assert!(matches!(hit, Some(Hit::Gap(gap)) if gap.path.is_empty() && !gap.down));
}

#[test]
fn a_press_in_a_card_padding_takes_the_nearest_cells_of_its_pane() {
    let layout = Layout::new(&side_by_side(), CELL, area(400., 200.));
    assert_eq!(layout.hit(point(px(190.), px(50.))), Some(Hit::Pane(2, (20, 2))));
}

#[test]
fn a_drag_out_of_a_pane_still_counts_in_its_cells() {
    let layout = Layout::new(&side_by_side(), CELL, area(400., 200.));
    assert_eq!(layout.cell_in(1, point(px(300.), px(-20.))), Some((36, -2)));
}

#[test]
fn the_ratio_of_a_drag_puts_the_gap_under_the_pointer() {
    let layout = Layout::new(&side_by_side(), CELL, area(400., 200.));
    let Some(Hit::Gap(gap)) = layout.hit(point(px(180.), px(50.))) else { panic!("the gap is there") };
    assert!((gap.along.ratio(180.) - 0.5).abs() < 1e-6);
}

#[test]
fn a_pane_beside_a_stack_takes_the_whole_height() {
    let layout = Layout::new(&beside_a_stack(), CELL, area(400., 400.));
    assert_eq!(bounds(&layout, 1).size.height, px(400.));
}

#[test]
fn a_stack_shares_its_width_and_floats_a_gap_apart() {
    let layout = Layout::new(&beside_a_stack(), CELL, area(400., 400.));
    let (top, bottom) = (bounds(&layout, 2), bounds(&layout, 3));
    assert_eq!((top.origin.x, top.right()), (bottom.origin.x, bottom.right()));
    assert_eq!(f32::from(bottom.origin.y - top.bottom()), GAP);
    assert_eq!(bottom.bottom(), px(400.));
}

#[test]
fn every_split_has_a_gap_named_by_its_path() {
    let layout = Layout::new(&beside_a_stack(), CELL, area(400., 400.));
    let gaps: Vec<(Vec<bool>, bool)> = layout.gaps.iter().map(|g| (g.path.clone(), g.down)).collect();
    assert_eq!(gaps, [(vec![], false), (vec![true], true)]);
}

#[test]
fn without_a_split_tree_panes_keep_their_places_on_one_grid() {
    let grid = Grid { layout: None, ..side_by_side() };
    let layout = Layout::new(&grid, CELL, area(400., 200.));
    assert_eq!(layout.card(2).map(|c| c.cells), Some(point(px(PADDING + 21. * CELL.0), px(PADDING))));
}

#[test]
fn a_span_of_cells_lies_on_its_row_inside_the_padding() {
    let grid = Grid { layout: Some(Node::Leaf(7)), panes: vec![pane(7, (0, 0), (10, 4))], ..Default::default() };
    let layout = Layout::new(&grid, CELL, area(200., 100.));
    let span = layout.span(7, 2, (3, 7)).expect("a span");
    assert_eq!(span.origin, point(px(PADDING + 3. * CELL.0), px(PADDING + 2. * CELL.1)));
    assert_eq!((span.size.width, span.size.height), (px(4. * CELL.0), px(CELL.1)));
    assert_eq!(layout.span(8, 0, (0, 1)), None);
}
