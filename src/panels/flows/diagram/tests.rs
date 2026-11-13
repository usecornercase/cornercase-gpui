use super::*;

#[test]
fn an_animation_counts_from_its_first_frame() {
    let (played, start) = (Played::default(), Instant::now());
    assert_eq!(played.since("run-7", "in-0".into(), start), Duration::ZERO);
    assert_eq!(played.since("run-7", "in-0".into(), start + TRAVEL), TRAVEL);
}

#[test]
fn a_played_animation_stays_played_while_its_element_is_away() {
    let (played, start) = (Played::default(), Instant::now());
    played.since("run-7", "in-0".into(), start);
    played.since("run-7", "travel-t-coder".into(), start + ENTER);
    assert_eq!(played.since("run-7", "in-0".into(), start + TRAVEL), TRAVEL);
}

#[test]
fn another_selection_plays_its_animations_again() {
    let (played, start) = (Played::default(), Instant::now());
    played.since("run-7", "in-0".into(), start);
    played.since("Quick", "in-0".into(), start + ENTER);
    assert_eq!(played.since("run-7", "in-0".into(), start + TRAVEL), Duration::ZERO);
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

#[test]
fn a_plan_that_fits_its_board_does_not_scroll() {
    assert!(near(sideways(418.4, 446.8), 0.));
}

#[test]
fn a_wider_plan_scrolls_by_what_it_does_not_show() {
    assert!(near(sideways(536.4, 446.8), 89.6));
}

#[test]
fn a_step_moves_most_of_the_shown_width_and_stops_at_the_end() {
    assert!(near(stepped(0., 200., 500., true), 160.));
    assert!(near(stepped(450., 200., 500., true), 500.));
    assert!(near(stepped(100., 200., 500., false), 0.));
}

#[test]
fn the_arrows_rest_at_the_ends_of_the_plan() {
    let most = 90.;
    assert!(!can_step(0., most, false));
    assert!(!can_step(most, most, true));
    assert!(can_step(most / 2., most, true));
    assert!(can_step(most / 2., most, false));
    assert!(sideways(100.5, 100.) <= SIDEWAYS_NEAR);
}
