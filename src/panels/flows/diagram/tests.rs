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
