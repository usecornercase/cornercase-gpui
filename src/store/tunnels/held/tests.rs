use super::{
    Again, Asked, Fate, Held, Hold, Settled, again_of, bring_back, came_back, fate as fate_of, forgets_after_close,
    forgets_failed, live, local_before, ports_kept, remembers, settle_now, settled, test_hold, unsent, up_toast,
    window_failure,
};
use crate::cli::{Ended, ended_after};
use crate::model::tunnels::Toward;

fn fate(hold: &Hold, code: Option<i32>, said: &str) -> Fate {
    fate_of(hold, &Ended { code, said: said.to_string(), last_line: String::new() })
}

fn printed(hold: &Hold, text: &str, code: i32) -> Fate {
    fate_of(hold, &ended_after(text, Some(code)))
}

fn hold(state: Held, was_up: bool) -> Hold {
    let mut hold = test_hold(Toward::Server, 5173, state);
    hold.boot = "boot-a".to_string();
    hold.was_up = was_up;
    hold
}

#[test]
fn a_hold_that_fails_before_it_was_up_is_said_and_not_retried() {
    let said = "Error: could not tunnel port 5173: ssh: connect to host devbox port 22: Connection refused\n";
    let fate = fate(&hold(Held::Opening, false), Some(1), said);
    assert_eq!(
        fate,
        Fate::Failed("Could not tunnel port 5173: ssh: connect to host devbox port 22: Connection refused".to_string())
    );
}

#[test]
fn a_hold_with_nothing_on_stderr_says_its_status() {
    let failed = fate(&hold(Held::Opening, false), Some(2), "");
    assert_eq!(
        failed,
        Fate::Failed("Could not tunnel port 5173: cornercase tunnel hold ended with status 2".to_string())
    );
    assert_eq!(window_failure(&Toward::Server, 5173, "no free port"), "Could not tunnel port 5173: no free port");
}

#[test]
fn a_hold_that_falls_after_it_was_up_is_retried() {
    let fate =
        fate(&hold(Held::Up { local: 5173 }, true), Some(1), "Error: the tunnel of port 5173 ended: ssh stopped");
    assert_eq!(fate, Fate::Retry("The tunnel of port 5173 ended: ssh stopped".to_string()));
}

#[test]
fn a_hold_this_window_ended_goes_quietly() {
    assert_eq!(fate(&hold(Held::Ending, true), Some(1), "Error: ssh stopped"), Fate::Gone);
    assert_eq!(fate(&hold(Held::Ending, true), Some(0), ""), Fate::Gone);
}

#[test]
fn an_exit_0_nobody_asked_for_is_settled_by_the_boot() {
    assert_eq!(fate(&hold(Held::Up { local: 5173 }, true), Some(0), ""), Fate::Settle);
}

#[test]
fn an_exit_0_on_the_same_boot_means_closed_there() {
    assert_eq!(settled("boot-a", "boot-a"), Settled::Closed);
}

#[test]
fn an_exit_0_on_a_new_boot_comes_back() {
    assert_eq!(settled("boot-a", "boot-b"), Settled::Back);
}

#[test]
fn remembered_ports_come_back_unless_held() {
    assert_eq!(bring_back(&[3000, 5173, 8080], &[5173]), vec![3000, 8080]);
    assert_eq!(bring_back(&[5173], &[5173]), Vec::<u16>::new());
}

#[test]
fn a_click_that_comes_up_says_which_local_port_and_offers_open() {
    let (text, open) = up_toast(Asked::Click, 5173, 5174, "devbox").expect("a toast");
    assert!(text.starts_with("Port 5173 on devbox is tunnelled to localhost:5174 (5173 is taken on this "), "{text}");
    assert!(open);
    let (text, open) = up_toast(Asked::Click, 5173, 5173, "devbox").expect("a toast");
    assert_eq!(text, "Port 5173 on devbox is tunnelled to localhost:5173");
    assert!(open);
}

#[test]
fn an_open_says_the_local_port_only_when_it_differs() {
    assert!(up_toast(Asked::Open, 5173, 5173, "devbox").is_none());
    let (text, open) = up_toast(Asked::Open, 5173, 5174, "devbox").expect("a toast");
    assert!(text.contains("localhost:5174") && !open);
}

#[test]
fn a_port_brought_back_says_nothing() {
    assert!(up_toast(Asked::Back, 5173, 5174, "devbox").is_none());
    assert!(up_toast(Asked::Retry, 5173, 5174, "devbox").is_none());
}

#[test]
fn an_exit_0_while_unlinked_waits_for_the_next_view_line() {
    assert_eq!(settle_now((false, false, true), "boot-a", Some("boot-a")), None);
    assert_eq!(settle_now((true, false, true), "boot-a", Some("boot-a")), Some(Settled::Closed));
    assert_eq!(settle_now((true, false, true), "boot-a", Some("boot-b")), Some(Settled::Back));
}

#[test]
fn a_hold_started_again_is_not_forgotten_by_the_exit_that_came_before() {
    assert_eq!(settle_now((true, true, false), "boot-a", Some("boot-a")), Some(Settled::Superseded));
}

#[test]
fn a_close_answered_after_the_port_was_opened_again_keeps_it() {
    assert!(!forgets_after_close(true));
    assert!(forgets_after_close(false));
}

#[test]
fn a_port_is_held_once_per_place() {
    let machine = Toward::Machine("vps".to_string());
    let holds =
        [test_hold(Toward::Server, 5173, Held::Up { local: 5173 }), test_hold(machine.clone(), 5173, Held::Opening)];
    assert_eq!(live(&holds, &Toward::Server, 5173).map(|h| h.state.clone()), Some(Held::Up { local: 5173 }));
    assert_eq!(live(&holds, &machine, 5173).map(|h| h.state.clone()), Some(Held::Opening));
    assert!(live(&holds, &Toward::Machine("other".to_string()), 5173).is_none());
    assert!(live(&holds, &Toward::Server, 80).is_none());
}

#[test]
fn a_machines_failure_names_it_and_hides_its_destination() {
    let machine = Toward::Machine("vps".to_string());
    let mut held = test_hold(machine.clone(), 8765, Held::Opening);
    held.was_up = false;
    let said = "Error: could not reach cornercase on `vps.invalid`: ssh: connect to host vps.invalid port 22: Connection refused\n";
    let Fate::Failed(text) = fate(&held, Some(1), said) else { panic!("a failure") };
    assert!(text.starts_with("Could not tunnel port 8765 on vps: could not reach cornercase on"), "{text}");
    assert!(!text.contains("vps.invalid"), "{text}");
    let said = "Error: could not tunnel port 8765: ssh: no route\n";
    let Fate::Failed(text) = fate(&held, Some(1), said) else { panic!("a failure") };
    assert_eq!(text, "Could not tunnel port 8765 on vps: ssh: no route");
    assert_eq!(window_failure(&machine, 8765, "x"), "Could not tunnel port 8765 on vps: x");
}

#[test]
fn a_machines_tunnel_is_labelled_by_its_name() {
    let (text, _) = up_toast(Asked::Click, 8765, 63235, "vps").expect("a toast");
    assert!(text.starts_with("Port 8765 on vps is tunnelled to localhost:63235 (8765 is taken on this "), "{text}");
    assert_eq!(test_hold(Toward::Machine("vps".to_string()), 1, Held::Opening).label(), "vps");
}

#[test]
fn a_machines_dropped_tunnel_reason_hides_its_destination() {
    let machine = Toward::Machine("vps".to_string());
    let mut held = test_hold(machine, 8765, Held::Up { local: 8765 });
    held.was_up = true;
    let said = "Error: the tunnel of port 8765 ended: ssh: connect to host vps.invalid port 22: Connection refused\n";
    let Fate::Retry(why) = fate(&held, Some(1), said) else { panic!("a retry") };
    assert!(why.contains("connect to host vps port 22") && !why.contains("vps.invalid"), "{why}");
}

#[test]
fn an_exit_0_waits_while_the_boot_to_compare_is_not_known() {
    assert_eq!(settle_now((true, false, true), "boot-a", None), None);
    assert_eq!(settle_now((true, true, true), "boot-a", None), None);
}

#[test]
fn a_same_boot_is_no_close_until_the_wait_has_passed_but_a_new_boot_is_a_restart_at_once() {
    assert_eq!(settle_now((true, false, false), "boot-a", Some("boot-a")), None);
    assert_eq!(settle_now((true, false, false), "boot-a", Some("boot-b")), Some(Settled::Back));
}

#[test]
fn a_programs_open_says_who_asked_and_where_the_port_is() {
    let (text, open) = up_toast(Asked::Program, 8765, 8765, "vps").expect("a toast");
    assert_eq!((text.as_str(), open), ("A program on vps tunnelled port 8765 to localhost:8765", true));
    let (moved, _) = up_toast(Asked::Program, 8765, 61011, "vps").expect("a toast");
    assert!(moved.starts_with("A program on vps tunnelled port 8765 to localhost:61011 (8765 is taken on this "));
    assert!(moved.ends_with(')'), "{moved}");
}

#[test]
fn an_asked_open_is_remembered_as_a_click_is() {
    assert!([Asked::Click, Asked::Open, Asked::Program].into_iter().all(remembers));
    assert!(![Asked::Back, Asked::Retry].into_iter().any(remembers));
}

#[test]
fn a_failed_hold_forgets_only_the_port_it_added() {
    let failed = Fate::Failed("no".to_string());
    assert!(forgets_failed(&failed, true));
    assert!(!forgets_failed(&failed, false));
    assert!(!forgets_failed(&Fate::Gone, true));
    assert!(!forgets_failed(&Fate::Retry("x".to_string()), true));
}

#[test]
fn a_restarted_hold_asks_for_the_local_port_it_had() {
    let mut up = hold(Held::Up { local: 61011 }, true);
    up.last_local = Some(61011);
    assert_eq!(again_of(&up).local, Some(61011));
    let mut failed = hold(Held::Opening, true);
    failed.had_local = Some(61011);
    assert_eq!(again_of(&failed).local, Some(61011));
    failed.attempts = 2;
    assert_eq!(again_of(&failed).attempts, 2);
}

#[test]
fn a_hold_brought_back_asks_for_the_remembered_local_port() {
    assert_eq!(local_before(None, Some(61011)), Some(61011));
    let again = Again { attempts: 1, local: Some(1) };
    assert_eq!(local_before(Some(again), Some(2)), Some(1));
    assert_eq!(local_before(Some(Again { attempts: 1, local: None }), Some(2)), Some(2));
}

#[test]
fn a_quiet_hold_that_came_back_elsewhere_says_where_and_why() {
    let here = crate::desktop::here();
    for asked in [Asked::Back, Asked::Retry] {
        assert_eq!(
            came_back(asked, (8765, 61023), Some(61011), "vps").as_deref(),
            Some(format!("Port 8765 on vps is back on localhost:61023 (localhost:61011, where it was, is taken on this {here})").as_str())
        );
        assert_eq!(
            came_back(asked, (8765, 8765), Some(61011), "vps").as_deref(),
            Some("Port 8765 on vps is back on localhost:8765 (it was on localhost:61011)")
        );
    }
}

#[test]
fn a_quiet_hold_back_where_it_was_says_nothing() {
    assert_eq!(came_back(Asked::Back, (8765, 61011), Some(61011), "vps"), None);
    assert_eq!(came_back(Asked::Retry, (8765, 61023), None, "vps"), None);
}

#[test]
fn a_click_keeps_its_own_words_when_it_moved() {
    assert_eq!(came_back(Asked::Click, (8765, 61023), Some(61011), "vps"), None);
    assert_eq!(came_back(Asked::Program, (8765, 61023), Some(61011), "vps"), None);
    assert_eq!(came_back(Asked::Open, (8765, 61023), Some(61011), "vps"), None);
}

#[test]
fn the_local_port_a_hold_came_up_on_wins_over_the_one_it_was_asked_for() {
    let mut both = hold(Held::Up { local: 61023 }, true);
    both.had_local = Some(55555);
    both.last_local = Some(61023);
    assert_eq!(again_of(&both).local, Some(61023));
    both.last_local = None;
    assert_eq!(again_of(&both).local, Some(55555));
}

#[test]
fn a_quiet_hold_that_was_on_its_own_port_says_where_it_landed_when_that_is_taken() {
    let here = crate::desktop::here();
    let said = came_back(Asked::Back, (8765, 61023), Some(8765), "vps");
    let words =
        format!("Port 8765 on vps is back on localhost:61023 (localhost:8765, where it was, is taken on this {here})");
    assert_eq!(said.as_deref(), Some(words.as_str()));
}

#[test]
fn a_port_pending_settlement_is_not_brought_back_with_the_link() {
    let toward = Toward::Server;
    let holds = vec![
        test_hold(Toward::Server, 5173, Held::Up { local: 1 }),
        test_hold(Toward::Machine("vps".into()), 9000, Held::Opening),
    ];
    let kept = ports_kept(&holds, &toward, [3000].into_iter());
    assert_eq!(kept, vec![5173, 3000]);
    assert_eq!(bring_back(&[3000, 5173, 8080], &kept), vec![8080]);
}

#[test]
fn a_hold_closed_there_ends_at_once() {
    let text = crate::test_util::fixture!("tunnels/hold_closed.txt");
    assert_eq!(printed(&hold(Held::Up { local: 61014 }, true), text, 0), Fate::Closed);
}

#[test]
fn a_hold_forgotten_there_ends_at_once() {
    let text = crate::test_util::fixture!("tunnels/hold_forgotten.txt");
    assert_eq!(printed(&hold(Held::Up { local: 61017 }, true), text, 0), Fate::Closed);
}

#[test]
fn a_hold_whose_server_stopped_falls_and_says_so() {
    let text = crate::test_util::fixture!("tunnels/hold_server_stopped.txt");
    let fate = printed(&hold(Held::Up { local: 61020 }, true), text, 7);
    assert_eq!(fate, Fate::Retry("Its server stopped".to_string()));
}

#[test]
fn a_hold_whose_server_restarted_falls_and_says_so() {
    let text = crate::test_util::fixture!("tunnels/hold_server_restarted.txt");
    let fate = printed(&hold(Held::Up { local: 8765 }, true), text, 7);
    assert_eq!(fate, Fate::Retry("Its server restarted".to_string()));
}

#[test]
fn a_hold_that_exits_7_with_no_line_falls_as_today() {
    let text = crate::test_util::fixture!("tunnels/held_line.txt");
    let fate = printed(&hold(Held::Up { local: 65433 }, true), text, 7);
    assert_eq!(fate, Fate::Retry("cornercase tunnel hold ended with status 7".to_string()));
}

#[test]
fn a_hold_that_exits_0_with_no_line_still_settles() {
    let text = crate::test_util::fixture!("tunnels/held_line.txt");
    assert_eq!(printed(&hold(Held::Up { local: 65433 }, true), text, 0), Fate::Settle);
}

#[test]
fn a_line_the_window_does_not_know_leaves_the_status_to_decide() {
    let text = crate::test_util::fixture!("tunnels/hold_end_unknown.txt");
    let up = hold(Held::Up { local: 61014 }, true);
    assert_eq!(printed(&up, text, 0), Fate::Settle);
    assert_eq!(printed(&up, text, 7), Fate::Retry("cornercase tunnel hold ended with status 7".to_string()));
}

#[test]
fn a_hold_that_never_came_up_and_was_closed_ends_at_once() {
    let text = crate::test_util::fixture!("tunnels/hold_end_alone.txt");
    assert_eq!(printed(&hold(Held::Opening, false), text, 0), Fate::Closed);
}

#[test]
fn a_hold_that_never_came_up_says_why_its_server_went() {
    let text = crate::test_util::fixture!("tunnels/hold_server_stopped.txt");
    let fate = printed(&hold(Held::Opening, false), text, 7);
    assert_eq!(fate, Fate::Failed("Could not tunnel port 5173: its server stopped".to_string()));
}

#[test]
fn a_hold_this_window_ended_goes_quietly_whatever_its_last_line() {
    let closed = crate::test_util::fixture!("tunnels/hold_closed.txt");
    let stopped = crate::test_util::fixture!("tunnels/hold_server_stopped.txt");
    assert_eq!(printed(&hold(Held::Ending, true), closed, 0), Fate::Gone);
    assert_eq!(printed(&hold(Held::Ending, true), stopped, 7), Fate::Gone);
}

#[test]
fn a_machines_hold_dropped_without_the_machine_is_noted_by_its_port_and_local_port() {
    let mut there = test_hold(Toward::Machine("vps".to_string()), 8765, Held::Up { local: 60950 });
    assert_eq!(unsent(&there), None);
    there.last_local = Some(60950);
    assert_eq!(unsent(&there), Some(("vps".to_string(), 8765, 60950)));
    let mut server = test_hold(Toward::Server, 8765, Held::Up { local: 60950 });
    server.last_local = Some(60950);
    assert_eq!(unsent(&server), None);
}
