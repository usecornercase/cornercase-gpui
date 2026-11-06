use super::{ARROWS_ROOM, Spot, can_step, followed_scroll, in_view, overflows, stepped};

const TABS: [(f32, f32); 4] = [(0., 100.), (104., 204.), (208., 308.), (312., 412.)];

fn spot(workspace: u64, tab: u64, width: f32, scrolled: f32) -> Spot {
    Spot { workspace: Some(workspace), tab: Some(tab), width, scrolled, kept: false }
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < f32::EPSILON
}

#[test]
fn arrows_show_as_soon_as_the_tabs_pass_the_edge() {
    assert!(overflows(1., false));
    assert!(!overflows(0., false));
}

#[test]
fn arrows_stay_while_the_tabs_would_not_fit_without_them() {
    assert!(overflows(ARROWS_ROOM + 1., true));
}

#[test]
fn arrows_go_once_the_tabs_fit_in_the_room_they_take() {
    assert!(!overflows(ARROWS_ROOM - 1., true));
}

#[test]
fn forward_brings_in_the_first_tab_past_the_edge() {
    assert!(near(stepped(&TABS, 250., 0., true), 58.));
}

#[test]
fn forward_goes_on_a_tab_at_a_time() {
    assert!(near(stepped(&TABS, 250., 58., true), 162.));
}

#[test]
fn back_brings_in_the_last_tab_before_the_edge() {
    assert!(near(stepped(&TABS, 250., 162., false), 104.));
}

#[test]
fn nothing_past_either_end_stays_where_it_is() {
    assert!(near(stepped(&TABS, 250., 162., true), 162.));
    assert!(near(stepped(&TABS, 250., 0., false), 0.));
}

#[test]
fn an_arrow_is_lit_while_there_is_something_that_way() {
    assert!(can_step(0., 100., true));
    assert!(!can_step(0., 100., false));
    assert!(can_step(100., 100., false));
    assert!(!can_step(100., 100., true));
}

#[test]
fn a_tab_past_the_right_edge_comes_in_by_its_right() {
    assert!(near(in_view((312., 412.), 250., 0.), 162.));
}

#[test]
fn a_tab_before_the_left_edge_comes_in_by_its_left() {
    assert!(near(in_view((104., 204.), 250., 162.), 104.));
}

#[test]
fn a_tab_in_view_scrolls_nothing() {
    assert!(near(in_view((104., 204.), 250., 58.), 58.));
}

#[test]
fn a_tab_wider_than_the_strip_shows_its_start() {
    assert!(near(in_view((104., 504.), 250., 58.), 104.));
    assert!(near(in_view((104., 504.), 250., 104.), 104.));
}

#[test]
fn another_tab_comes_into_view() {
    let next = followed_scroll(spot(1, 1, 250., 0.), spot(1, 4, 250., 0.), Some((312., 412.)));
    assert!(near(next, 162.));
}

#[test]
fn scrolling_away_is_left_alone_while_the_tab_stays_the_same() {
    let was = Spot { kept: true, ..spot(1, 1, 250., 0.) };
    assert!(near(followed_scroll(was, spot(1, 1, 250., 162.), Some((0., 100.))), 162.));
    assert!(near(followed_scroll(spot(1, 1, 250., 162.), spot(1, 1, 250., 162.), Some((0., 100.))), 162.));
}

#[test]
fn a_shown_tab_that_grows_past_the_edge_stays_in_view() {
    let was = Spot { kept: true, ..spot(1, 4, 250., 162.) };
    assert!(near(followed_scroll(was, spot(1, 4, 250., 162.), Some((312., 440.))), 190.));
}

#[test]
fn a_tab_scrolled_away_does_not_come_back_when_it_grows() {
    assert!(near(followed_scroll(spot(1, 4, 250., 0.), spot(1, 4, 250., 0.), Some((312., 440.))), 0.));
}

#[test]
fn another_workspace_starts_over_at_the_first_tab() {
    let next = followed_scroll(spot(1, 1, 250., 162.), spot(2, 5, 250., 162.), Some((0., 100.)));
    assert!(near(next, 0.));
}

#[test]
fn another_workspace_starts_over_before_following_its_tab() {
    let next = followed_scroll(spot(1, 1, 90., 162.), spot(2, 5, 250., 162.), Some((104., 204.)));
    assert!(near(next, 0.));
}

#[test]
fn a_narrower_strip_keeps_the_active_tab_in_view() {
    let next = followed_scroll(spot(1, 2, 410., 0.), spot(1, 2, 150., 0.), Some((104., 204.)));
    assert!(near(next, 54.));
}

#[test]
fn no_tab_shown_scrolls_nothing() {
    let was = spot(1, 1, 250., 58.);
    assert!(near(followed_scroll(was, Spot { tab: None, ..was }, None), 58.));
}
