use std::time::{Duration, Instant};

use super::{
    Beat, Leaving, Next, ORBIT, ORBIT_FRAME, Way, ease_in, ease_out, next_frame, orbit, orbit_turned, orbits_drawn,
};

#[test]
fn a_closed_item_stays_while_it_leaves() {
    let mut leaving = Leaving::default();
    assert_eq!(leaving.track(Some("changes")), None);
    let serial = leaving.track(None).expect("closing starts a leave");
    assert_eq!(leaving.gone(), Some((serial, &"changes")));
    assert!(leaving.finish(serial));
    assert_eq!(leaving.gone(), None);
}

#[test]
fn nothing_leaves_when_nothing_was_shown() {
    let mut leaving: Leaving<&str> = Leaving::default();
    assert_eq!(leaving.track(None), None);
    assert_eq!(leaving.gone(), None);
}

#[test]
fn reopening_cancels_the_leave() {
    let mut leaving = Leaving::default();
    leaving.track(Some("files"));
    let serial = leaving.track(None).expect("closing starts a leave");
    leaving.track(Some("files"));
    assert_eq!(leaving.gone(), None);
    assert!(!leaving.finish(serial));
}

#[test]
fn an_old_timer_does_not_end_a_newer_leave() {
    let mut leaving = Leaving::default();
    leaving.track(Some("todo"));
    let first = leaving.track(None).expect("first leave");
    leaving.track(Some("todo"));
    let second = leaving.track(None).expect("second leave");
    assert!(!leaving.finish(first));
    assert_eq!(leaving.gone(), Some((second, &"todo")));
}

#[test]
fn repeated_updates_while_closed_keep_the_leave() {
    let mut leaving = Leaving::default();
    leaving.track(Some("todo"));
    let serial = leaving.track(None).expect("closing starts a leave");
    assert_eq!(leaving.track(None), None);
    assert_eq!(leaving.gone(), Some((serial, &"todo")));
}

#[test]
fn easings_start_at_zero_and_end_at_one() {
    for ease in [ease_in, ease_out] {
        assert!(ease(0.).abs() < f32::EPSILON);
        assert!((ease(1.) - 1.).abs() < f32::EPSILON);
    }
    assert!(ease_out(0.5) > 0.5 && ease_in(0.5) < 0.5);
}

#[test]
fn an_item_is_fully_shown_at_the_end_of_entering_and_the_start_of_leaving() {
    assert!((Way::In.shown(1.) - 1.).abs() < f32::EPSILON);
    assert!((Way::Pop.shown(1.) - 1.).abs() < f32::EPSILON);
    assert!((Way::Out.shown(0.) - 1.).abs() < f32::EPSILON);
    assert!(Way::Out.shown(1.).abs() < f32::EPSILON);
}

#[test]
fn the_orbit_turns_once_a_period_and_comes_back_round() {
    assert!(orbit_turned(Duration::ZERO).abs() < f32::EPSILON);
    assert!((orbit_turned(ORBIT / 4) - 0.25).abs() < 1e-6);
    assert!(orbit_turned(ORBIT).abs() < f32::EPSILON);
    assert!((orbit_turned(ORBIT * 3 + ORBIT / 2) - 0.5).abs() < 1e-6);
}

#[test]
fn the_orbit_turns_smoothly_rather_than_in_steps() {
    let at = ORBIT * 7 + Duration::from_millis(400);
    let step = orbit_turned(at + Duration::from_millis(1)) - orbit_turned(at);
    assert!(step > 0. && step < 0.001, "{step}");
}

fn frames_between_orbit_frames(hertz: u32) -> u32 {
    let display = Duration::from_secs(1) / hertz;
    (1..=hertz).find(|frames| next_frame(display * *frames, display, 1) == Next::Draw).expect("a draw within a second")
}

#[test]
fn the_orbits_draw_on_the_display_frame_nearest_a_thirtieth_of_a_second() {
    let frames: Vec<u32> = [60, 90, 120, 144, 240].into_iter().map(frames_between_orbit_frames).collect();
    assert_eq!(frames, [2, 3, 4, 5, 8]);
}

#[test]
fn nothing_is_drawn_for_the_orbits_while_none_shows() {
    assert_eq!(next_frame(ORBIT, ORBIT, 0), Next::Rest);
}

#[test]
fn a_frame_drawn_for_another_reason_puts_the_next_orbit_frame_off() {
    let beat = Beat::default();
    let start = Instant::now();
    let display = Duration::from_micros(8_333);
    beat.drawing(start);
    assert_eq!(beat.tick(start + display, 1), Next::Wait);
    beat.drawing(start + display * 2);
    let ticks: Vec<Next> = (3..=6).map(|frame| beat.tick(start + display * frame, 1)).collect();
    assert_eq!(ticks, [Next::Wait, Next::Wait, Next::Wait, Next::Draw]);
}

#[test]
fn a_beat_runs_once_until_it_draws_or_rests() {
    let beat = Beat::default();
    let start = Instant::now();
    beat.drawing(start);
    assert!(beat.start());
    assert!(!beat.start());
    assert_eq!(beat.tick(start + ORBIT_FRAME, 1), Next::Draw);
    assert!(beat.start());
    assert_eq!(beat.tick(start + ORBIT_FRAME, 0), Next::Rest);
    assert!(beat.start());
}

#[test]
fn a_new_frame_counts_its_orbits_afresh() {
    let beat = Beat::default();
    let _ = orbit(gpui::svg());
    beat.drawing(Instant::now());
    assert_eq!(orbits_drawn(), 0);
    let _ = orbit(gpui::svg());
    let _ = orbit(gpui::svg());
    assert_eq!(orbits_drawn(), 2);
}
