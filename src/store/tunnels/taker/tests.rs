use std::time::Duration;

use super::{
    Answer, Cause, MOST_ASKED, Next, Phase, answer, keeps, may_take, next, orders_stay, refusal_words, split_closed,
    tells, unheard, untold, wait_after,
};
use crate::model::tunnels::{Gate, Older, Order, Toward};
use crate::store::tunnels::held::{Held, test_hold};
use crate::store::tunnels::links::{Link, hold_stays};

const G: u64 = 7;
const RUNNING: Option<Phase> = Some(Phase::Running);
const WAITING: Option<Phase> = Some(Phase::Waiting { generation: G });

#[test]
fn a_destination_takes_orders_once_linked_and_new_enough() {
    assert_eq!(next(None, Cause::Follow, (true, true)), Next::Start);
    assert_eq!(next(None, Cause::Follow, (false, true)), Next::Nothing);
}

#[test]
fn a_taker_is_never_started_twice() {
    for cause in [Cause::Follow, Cause::Linked, Cause::Due { generation: G }] {
        assert_eq!(next(RUNNING, cause, (true, true)), Next::Nothing, "{cause:?}");
    }
}

#[test]
fn a_taker_that_ended_waits_its_turn() {
    assert_eq!(next(WAITING, Cause::Follow, (true, true)), Next::Nothing);
    assert_eq!(next(WAITING, Cause::Due { generation: G }, (true, true)), Next::Start);
    assert_eq!(next(WAITING, Cause::Due { generation: G + 1 }, (true, true)), Next::Nothing);
    assert_eq!(next(WAITING, Cause::Due { generation: G }, (false, true)), Next::Stop);
}

#[test]
fn a_new_link_attaches_at_once() {
    assert_eq!(next(WAITING, Cause::Linked, (true, true)), Next::Start);
    assert_eq!(next(WAITING, Cause::Linked, (false, true)), Next::Nothing);
}

#[test]
fn a_stopped_takers_timer_starts_nothing() {
    assert_eq!(next(None, Cause::Due { generation: G }, (true, true)), Next::Nothing);
}

#[test]
fn a_taker_ends_where_its_holds_end() {
    for phase in [RUNNING, WAITING] {
        assert_eq!(next(phase, Cause::Follow, (true, false)), Next::Stop);
    }
}

fn up(destination: &str) -> Link {
    Link::Up { boot: "b".to_string(), destination: destination.to_string() }
}

#[test]
fn a_taker_stays_with_its_link() {
    let stays = |link: &Link| orders_stay(hold_stays("vps.invalid", link), Gate::Open);
    assert!(!stays(&Link::Lost));
    assert!(!stays(&up("other.invalid")));
    assert!(!stays(&Link::Waiting(Some("other.invalid".to_string()))));
    assert!(stays(&Link::Waiting(None)));
    assert!(stays(&up("vps.invalid")));
}

#[test]
fn a_taker_ends_when_a_side_is_known_older() {
    assert!(!orders_stay(true, Gate::Older(Older::ThisComputer(None))));
    assert!(!orders_stay(true, Gate::Older(Older::Server(Some(crate::cli::Version::new(0, 13, 56))))));
    assert!(orders_stay(true, Gate::Older(Older::Server(None))));
}

#[test]
fn the_wait_backs_off_like_the_holds() {
    let seconds: Vec<u64> = (1..=5).map(|attempts| wait_after(attempts).as_secs()).collect();
    assert_eq!(seconds, [1, 2, 4, 8, 15]);
    assert_eq!(wait_after(9), Duration::from_secs(15));
}

#[test]
fn one_destination_has_one_taker() {
    assert!(!may_take(Gate::Open, true, true));
    assert!(may_take(Gate::Open, true, false));
    assert!(!may_take(Gate::Waiting, true, false));
    assert!(!may_take(Gate::Older(Older::Server(None)), true, false));
    assert!(!may_take(Gate::Open, false, false));
}

fn asked(held: Option<&Held>, live: usize, cannot: Option<&str>) -> Answer {
    answer(held, live, cannot.map(str::to_string), (8765, "vps"))
}

#[test]
fn an_open_for_each_state_of_its_port() {
    assert_eq!(asked(None, 0, None), Answer::Hold);
    assert_eq!(asked(Some(&Held::Opening), 1, None), Answer::Ignore);
    assert_eq!(asked(Some(&Held::Up { local: 1 }), 1, None), Answer::Ignore);
    assert_eq!(asked(Some(&Held::Down { why: String::new() }), 1, None), Answer::Again);
    let Answer::Refuse(words) = asked(Some(&Held::Ending), 1, None) else { panic!("a refusal") };
    assert!(words.contains("is closing the tunnel of port 8765"), "{words}");
}

#[test]
fn the_window_says_at_once_why_it_will_not_hold() {
    let cannot = Some("vps is no longer listed");
    for held in [None, Some(&Held::Down { why: String::new() })] {
        let Answer::Refuse(words) = asked(held, 0, cannot) else { panic!("a refusal") };
        assert!(words.contains("cannot hold it now: vps is no longer listed"), "{words}");
    }
    assert_eq!(asked(Some(&Held::Up { local: 1 }), 0, cannot), Answer::Ignore);
}

#[test]
fn asks_stop_at_32_tunnels_to_one_place() {
    let Answer::Refuse(words) = asked(None, MOST_ASKED, None) else { panic!("a refusal") };
    assert!(words.contains("already holds 32 tunnels to vps"), "{words}");
    assert_eq!(asked(None, MOST_ASKED - 1, None), Answer::Hold);
    assert_eq!(asked(Some(&Held::Down { why: String::new() }), MOST_ASKED, None), Answer::Again);
}

#[test]
fn a_close_order_ends_that_ports_holds_but_one_being_closed_here() {
    let vps = || Toward::Machine("vps".to_string());
    let holds = vec![
        test_hold(vps(), 8765, Held::Opening),
        test_hold(vps(), 8765, Held::Up { local: 1 }),
        test_hold(vps(), 8765, Held::Down { why: String::new() }),
        test_hold(vps(), 8765, Held::Ending),
        test_hold(Toward::Server, 8765, Held::Up { local: 1 }),
        test_hold(Toward::Machine("lab".to_string()), 8765, Held::Up { local: 1 }),
        test_hold(vps(), 9000, Held::Up { local: 1 }),
    ];
    let (ended, kept) = split_closed(holds, &vps(), 8765);
    assert_eq!(ended.len(), 3);
    let states: Vec<&Held> = kept.iter().map(|hold| &hold.state).collect();
    assert_eq!(kept.len(), 4);
    assert!(states.contains(&&Held::Ending));
}

#[test]
fn an_empty_reason_still_says_why() {
    let words = refusal_words(" \n ");
    assert!(words.starts_with("the window on this ") && words.ends_with(" cannot hold it"), "{words}");
    assert_eq!(refusal_words("closing"), "closing");
}

#[test]
fn an_asked_open_is_kept_and_a_close_or_forget_is_not() {
    assert!(keeps(Order::Open(8765)));
    assert!(!keeps(Order::Close(8765)));
    assert!(!keeps(Order::Forget(8765)));
}

#[test]
fn a_failing_taker_is_told_once_after_five_ends() {
    assert!(!tells(4, false));
    assert!(tells(5, false));
    assert!(tells(9, false));
    assert!(!tells(5, true));
}

#[test]
fn the_words_for_a_taker_that_cannot_attach_hide_a_machines_address() {
    let said = "Error: could not reach cornercase on `vps.invalid`: ssh stopped\n";
    let machine = Toward::Machine("vps".to_string());
    let text = unheard(&machine, ("vps", "vps.invalid"), said);
    assert_eq!(
        text,
        "A program on vps cannot ask this window for a tunnel: could not reach cornercase on `vps`: ssh stopped"
    );
    let server = unheard(&Toward::Server, ("devbox", "devbox"), "");
    assert_eq!(server, "A program on devbox cannot ask this window for a tunnel: the connection ended");
    assert_eq!(
        untold(&Toward::Server, ("devbox", "devbox"), 8765, "x"),
        "Could not tell devbox that port 8765 is not tunnelled here: x"
    );
}
