use super::{Asked, Fate, Held, Hold, Settled, bring_back, failed_toast, fate, settled, up_toast};

fn hold(state: Held, was_up: bool) -> Hold {
    let mut hold = Hold::opening(5173, 1, Asked::Click, "boot-a");
    hold.state = state;
    hold.was_up = was_up;
    hold
}

#[test]
fn a_hold_that_fails_before_it_was_up_is_said_and_not_retried() {
    let said = "Error: could not tunnel port 5173: ssh: connect to host devbox port 22: Connection refused\n";
    let fate = fate(&hold(Held::Opening, false), Some(1), said);
    let Fate::Failed(why) = fate else { panic!("failed, got {fate:?}") };
    assert_eq!(
        failed_toast(5173, &why),
        "Could not tunnel port 5173: ssh: connect to host devbox port 22: Connection refused"
    );
}

#[test]
fn a_hold_with_nothing_on_stderr_says_its_status() {
    let Fate::Failed(why) = fate(&hold(Held::Opening, false), Some(2), "") else { panic!("failed") };
    assert_eq!(failed_toast(5173, &why), "Could not tunnel port 5173: cornercase tunnel hold ended with status 2");
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
