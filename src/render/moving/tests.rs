use std::time::{Duration, Instant};

use gpui::{point, px, size};

use super::*;
use crate::model::{Grid, Node};
use crate::render::fixtures::{CELL, area, bounds, pane, side_by_side, split};

fn at(x: f32, y: f32) -> Point<Pixels> {
    point(px(x), px(y))
}

fn later(start: Instant, ms: u64) -> Instant {
    start + Duration::from_millis(ms)
}

fn laid(grid: &Grid) -> Layout {
    Layout::new(grid, CELL, area(400., 200.))
}

fn swapped() -> Grid {
    Grid {
        layout: Some(split("right", Node::Leaf(2), Node::Leaf(1))),
        panes: vec![pane(2, (0, 0), (19, 10)), pane(1, (21, 0), (19, 10))],
        ..side_by_side()
    }
}

fn pressed(on: u64, grip: Grip) -> Moves {
    let mut moves = Moves::default();
    moves.press(Press { pane: on, grip, cell: (3, 4), at: at(88., 9.) });
    moves
}

fn dragged_to(to: Point<Pixels>, layout: &Layout, now: Instant) -> Moves {
    let mut moves = pressed(1, Grip::Handle);
    moves.drag(to, layout, now);
    moves
}

fn dropped(layout: &Layout, now: Instant) -> Moves {
    let mut moves = dragged_to(at(395., 100.), layout, now);
    let release = moves.release(side_by_side().layout, later(now, 400));
    assert_eq!(release, Release::Drop(1, Aim { target: 2, place: Place::Right }));
    moves
}

#[test]
fn a_pane_dropped_in_the_middle_swaps_else_takes_the_nearest_edge() {
    let card = Bounds::new(at(100., 0.), size(px(300.), px(120.)));
    assert_eq!(place_at(card, at(250., 60.)), Place::Swap);
    assert_eq!(place_at(card, at(110., 60.)), Place::Left);
    assert_eq!(place_at(card, at(390., 50.)), Place::Right);
    assert_eq!(place_at(card, at(240., 2.)), Place::Above);
    assert_eq!(place_at(card, at(240., 118.)), Place::Below);
}

#[test]
fn each_place_lights_its_side_of_the_card_inside_its_edge() {
    let card = Bounds::new(at(0., 0.), size(px(200.), px(100.)));
    assert_eq!(Place::Swap.zone(card), rect(6., 6., 188., 88.));
    assert_eq!(Place::Left.zone(card), rect(6., 6., 88., 88.));
    assert_eq!(Place::Right.zone(card), rect(106., 6., 88., 88.));
    assert_eq!(Place::Below.zone(card), rect(6., 56., 188., 38.));
}

#[test]
fn the_handle_shows_only_near_the_top_of_a_card_among_several() {
    let two = laid(&side_by_side());
    assert_eq!(near_top(&two, at(250., 20.)), Some(2));
    assert_eq!(near_top(&two, at(250., 120.)), None);
    let alone = Grid { layout: Some(Node::Leaf(7)), panes: vec![pane(7, (0, 0), (10, 4))], ..Default::default() };
    assert_eq!(near_top(&laid(&alone), at(200., 10.)), None);
}

#[test]
fn the_handle_sits_centred_in_the_top_padding() {
    let card = Bounds::new(at(100., 50.), size(px(300.), px(120.)));
    assert_eq!(handle(card), rect(214., 54., 72., 10.));
}

#[test]
fn a_press_lifts_the_pane_only_past_a_few_points() {
    let (layout, start) = (laid(&side_by_side()), Instant::now());
    let mut moves = pressed(1, Grip::Handle);
    moves.drag(at(90., 11.), &layout, start);
    assert!(!moves.lifted());
    moves.drag(at(96., 15.), &layout, start);
    assert!(moves.lifted());
}

#[test]
fn a_pane_alone_in_its_tab_never_lifts() {
    let alone = Grid { layout: Some(Node::Leaf(1)), panes: vec![pane(1, (0, 0), (10, 4))], ..Default::default() };
    let moves = dragged_to(at(300., 150.), &laid(&alone), Instant::now());
    assert!(!moves.lifted());
}

#[test]
fn a_release_before_lifting_is_a_click() {
    let mut moves = pressed(1, Grip::Right);
    let press = moves.pressed().expect("pressed");
    assert_eq!(moves.release(None, Instant::now()), Release::Click(press));
    assert_eq!(moves.pressed(), None);
}

#[test]
fn a_lifted_pane_aims_at_the_card_under_the_pointer() {
    let (layout, start) = (laid(&side_by_side()), Instant::now());
    let mut moves = dragged_to(at(292., 100.), &layout, start);
    assert_eq!(moves.release(None, start), Release::Drop(1, Aim { target: 2, place: Place::Swap }));
}

#[test]
fn a_pane_dropped_on_its_own_card_goes_home() {
    let (layout, start) = (laid(&side_by_side()), Instant::now());
    let mut moves = dragged_to(at(200., 100.), &layout, start);
    moves.drag(at(100., 100.), &layout, start);
    assert_eq!(moves.release(None, start), Release::Home);
}

#[test]
fn crossing_a_gap_keeps_the_last_aim() {
    let (layout, start) = (laid(&side_by_side()), Instant::now());
    let mut moves = dragged_to(at(200., 100.), &layout, start);
    moves.drag(at(180., 100.), &layout, start);
    assert_eq!(moves.release(None, start), Release::Drop(1, Aim { target: 2, place: Place::Left }));
}

#[test]
fn the_zone_grows_in_then_glides_from_where_it_is_to_the_new_place() {
    let (layout, start) = (laid(&side_by_side()), Instant::now());
    let mut moves = dragged_to(at(200., 100.), &layout, start);
    let left = Place::Left.zone(bounds(&layout, 2));
    assert_eq!(moves.stage(&layout, start).zone.map(|zone| zone.shown.alpha), Some(0.));
    let turned = later(start, 500);
    moves.drag(at(395., 100.), &layout, turned);
    assert_eq!(moves.stage(&layout, turned).zone.map(|zone| zone.shown.rect), Some(left));
    let right = Place::Right.zone(bounds(&layout, 2));
    let settled = moves.stage(&layout, later(turned, 200)).zone.expect("a zone");
    assert_eq!((settled.shown.rect, settled.shown.alpha, settled.place), (right, 1., Place::Right));
}

#[test]
fn the_ghost_leaves_its_card_and_hangs_under_the_pointer() {
    let (layout, start) = (laid(&side_by_side()), Instant::now());
    let moves = dragged_to(at(300., 150.), &layout, start);
    let leaving = moves.ghost(&layout, start).expect("a ghost");
    assert_eq!((leaving.rect, leaving.scale), (bounds(&layout, 1), 1.));
    let hanging = moves.ghost(&layout, later(start, 1000)).expect("a ghost");
    assert_eq!(hanging.rect.center().x, px(300.));
    assert!(hanging.rect.origin.y < px(150.) && hanging.scale < 0.5);
}

#[test]
fn the_lifted_card_turns_hollow_while_the_ghost_leaves_it() {
    let (layout, start) = (laid(&side_by_side()), Instant::now());
    let moves = dragged_to(at(300., 150.), &layout, start);
    assert_eq!(moves.stage(&layout, start).hollow, Some((1, 0.)));
    assert_eq!(moves.stage(&layout, later(start, 1000)).hollow, Some((1, 1.)));
}

#[test]
fn a_dropped_pane_waits_hollow_for_cornercase() {
    let (layout, start) = (laid(&side_by_side()), Instant::now());
    let moves = dropped(&layout, start);
    assert_eq!(moves.stage(&layout, later(start, 600)).hollow, Some((1, 1.)));
}

#[test]
fn the_new_layout_slides_the_other_cards_from_where_they_were() {
    let (before, after, start) = (laid(&side_by_side()), laid(&swapped()), Instant::now());
    let mut moves = dropped(&before, start);
    let landed = later(start, 450);
    moves.laid_out(&before, &after, landed);
    let stage = moves.stage(&after, landed);
    assert_eq!((stage.cards, stage.hidden), (vec![(2, bounds(&before, 2))], Some((1, 0.))));
    assert_eq!(moves.stage(&after, later(landed, 400)).cards, vec![(2, bounds(&after, 2))]);
}

#[test]
fn the_ghost_lands_on_the_moved_pane_and_lights_it() {
    let (before, after, start) = (laid(&side_by_side()), laid(&swapped()), Instant::now());
    let mut moves = dropped(&before, start);
    moves.laid_out(&before, &after, later(start, 450));
    let landing = moves.ghost(&after, later(start, 830)).expect("a ghost");
    assert_eq!((landing.rect, landing.scale, landing.alpha), (bounds(&after, 1), 1., 0.));
    moves.tick(later(start, 840));
    assert_eq!(
        (moves.ghost(&after, later(start, 840)), moves.stage(&after, later(start, 840)).glow),
        (None, Some((1, 1.)))
    );
}

#[test]
fn a_refused_move_flies_home_through_a_red_zone() {
    let (layout, start) = (laid(&side_by_side()), Instant::now());
    let mut moves = dropped(&layout, start);
    let refused = later(start, 450);
    moves.answered(&Answer::Refused, refused);
    assert!(moves.stage(&layout, refused).zone.is_some_and(|zone| zone.refused));
    moves.tick(later(refused, 300));
    assert_eq!(
        (moves.ghost(&layout, later(refused, 300)), moves.stage(&layout, later(refused, 300)).hollow),
        (None, None)
    );
}

#[test]
fn a_move_that_changes_nothing_flies_home() {
    let (layout, start) = (laid(&side_by_side()), Instant::now());
    let mut moves = dropped(&layout, start);
    moves.answered(&Answer::Moved(swapped().layout), later(start, 450));
    assert_eq!(moves.stage(&layout, later(start, 1000)).hollow, Some((1, 1.)));
    moves.answered(&Answer::Moved(side_by_side().layout), later(start, 1000));
    assert_eq!(moves.stage(&layout, later(start, 1300)).hollow, Some((1, 0.)));
}

#[test]
fn a_move_from_the_menu_slides_every_card_and_lights_the_moved_one() {
    let (before, after, start) = (laid(&side_by_side()), laid(&swapped()), Instant::now());
    let mut moves = Moves::default();
    moves.expect(1, side_by_side().layout, start);
    moves.laid_out(&before, &after, later(start, 30));
    let stage = moves.stage(&after, later(start, 30));
    assert_eq!(stage.cards, vec![(2, bounds(&before, 2)), (1, bounds(&before, 1))]);
    assert_eq!(stage.glow, Some((1, 1.)));
}

#[test]
fn escape_sends_a_lifted_pane_home_and_forgets_a_press() {
    let (layout, start) = (laid(&side_by_side()), Instant::now());
    let mut moves = dragged_to(at(292., 100.), &layout, start);
    assert!(moves.cancel(later(start, 500)) && moves.flying() && !moves.lifted());
    let mut press = pressed(1, Grip::Right);
    assert!(press.cancel(start) && press.pressed().is_none() && !press.flying());
    assert!(!Moves::default().cancel(start));
}

#[test]
fn frames_stop_once_every_animation_has_ended() {
    let (before, after, start) = (laid(&side_by_side()), laid(&swapped()), Instant::now());
    let mut moves = dropped(&before, start);
    moves.laid_out(&before, &after, later(start, 450));
    moves.tick(later(start, 600));
    assert!(moves.busy(later(start, 600)));
    moves.tick(later(start, 900));
    assert!(moves.busy(later(start, 900)));
    moves.tick(later(start, 2000));
    assert!(!moves.busy(later(start, 2000)));
}

#[test]
fn a_shuffled_card_carries_its_cells_with_it() {
    let layout = laid(&side_by_side());
    let card = *layout.card(2).expect("a card");
    let stage = Stage { cards: vec![(2, rect(10., 20., 100., 50.))], ..Stage::default() };
    let (shown, look) = stage.shown(&card);
    assert_eq!((shown.cells, look), (at(10. + 12., 20. + 12.), Look { shown: 1., hollow: 0. }));
}

#[test]
fn an_answer_reads_the_new_layout_or_a_refusal() {
    let moved = Answer::read(&Ok(r#"{"layout":{"leaf":4},"tab":3}"#.to_string()));
    assert_eq!(moved, Answer::Moved(Some(Node::Leaf(4))));
    assert_eq!(Answer::read(&Err(anyhow::anyhow!("no room"))), Answer::Refused);
    assert_eq!(Answer::read(&Ok("moved".to_string())), Answer::Moved(None));
}

#[test]
fn the_ghost_carries_the_place_it_would_land_on() {
    let (layout, start) = (laid(&side_by_side()), Instant::now());
    let moves = dragged_to(at(292., 100.), &layout, start);
    let place = moves.ghost(&layout, later(start, 500)).and_then(|ghost| ghost.zone).map(|zone| zone.place);
    assert_eq!(place, Some(Place::Swap));
}

#[test]
fn a_right_press_lifts_only_once_the_pointer_leaves_its_card() {
    let (layout, start) = (laid(&side_by_side()), Instant::now());
    let mut moves = pressed(1, Grip::Right);
    moves.drag(at(150., 100.), &layout, start);
    assert!(!moves.lifted());
    moves.drag(at(300., 100.), &layout, start);
    assert!(moves.lifted());
}

#[test]
fn a_right_release_on_another_pane_moves_it_without_a_move_in_between() {
    let (layout, start) = (laid(&side_by_side()), Instant::now());
    let mut moves = pressed(1, Grip::Right);
    moves.drag(at(395., 100.), &layout, start);
    assert_eq!(moves.release(None, start), Release::Drop(1, Aim { target: 2, place: Place::Right }));
    assert!(moves.flying());
}

#[test]
fn a_right_release_outside_the_panes_leaves_no_ghost_behind() {
    let (layout, start) = (laid(&side_by_side()), Instant::now());
    let mut moves = pressed(1, Grip::Right);
    moves.drag(at(-40., 100.), &layout, start);
    assert_eq!((moves.release(None, start), moves.flying()), (Release::Home, false));
}
