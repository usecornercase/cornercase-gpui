use super::*;
use crate::cli::Remote;
use crate::store::OlderNote;

fn remote(destination: &str) -> Target {
    Target::Remote(Remote::new(destination, "").expect("a destination"))
}

#[test]
fn a_machine_the_session_no_longer_lists_is_dropped() {
    assert_eq!(follow_of(None, Some(&remote("vps")), true), Follow::Drop);
}

#[test]
fn an_up_machine_already_followed_on_the_same_target_is_kept() {
    let target = remote("vps");
    assert_eq!(follow_of(Some(&Reach::Up(target.clone())), Some(&target), false), Follow::Keep);
}

#[test]
fn an_up_machine_with_another_destination_is_followed_again() {
    let (now, was) = (remote("vps2"), remote("vps"));
    assert_eq!(follow_of(Some(&Reach::Up(now.clone())), Some(&was), true), Follow::Start(now));
}

#[test]
fn an_up_machine_not_followed_starts_when_due_and_waits_before() {
    let reach = Reach::Up(remote("vps"));
    assert_eq!(follow_of(Some(&reach), None, true), Follow::Start(remote("vps")));
    assert_eq!(follow_of(Some(&reach), None, false), Follow::Wait);
}

#[test]
fn a_machine_that_cannot_be_reached_stops_its_streams() {
    let reaches = [
        Reach::Down("vps unreachable".into()),
        Reach::Older(OlderNote::default()),
        Reach::Refused("vps is linked from devbox".into()),
        Reach::Waiting("waiting for the machines list".into()),
    ];
    for reach in &reaches {
        assert_eq!(follow_of(Some(reach), Some(&remote("vps")), true), Follow::Stop, "{reach:?}");
    }
}

#[test]
fn the_events_start_with_a_listed_bot_and_stop_with_none() {
    assert_eq!(events_wanted(true, false), Events::Start);
    assert_eq!(events_wanted(false, true), Events::Stop);
    assert_eq!((events_wanted(true, true), events_wanted(false, false)), (Events::Leave, Events::Leave));
}

#[test]
fn a_machines_note_is_the_reason_it_cannot_be_reached() {
    let down = Reach::Down("vps unreachable".into());
    assert_eq!(note_of("vps", Some(&down), false).as_deref(), Some("vps unreachable"));
}

#[test]
fn an_up_machine_whose_stream_ended_is_reconnecting() {
    let up = Reach::Up(remote("vps"));
    assert_eq!(note_of("vps", Some(&up), true).as_deref(), Some("vps did not answer · reconnecting…"));
    assert_eq!(note_of("vps", Some(&up), false), None);
    assert_eq!(note_of("vps", None, true), None);
}
