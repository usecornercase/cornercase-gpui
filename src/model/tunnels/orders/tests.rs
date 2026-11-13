use std::ffi::OsStr;

use super::*;
use crate::cli::{Remote, Target};
use crate::test_util::fixture;

const TAKING: &str = fixture!("tunnels/orders_taking.json");
const TAKING_HELD: &str = fixture!("tunnels/orders_taking_held.json");
const OPEN: &str = fixture!("tunnels/order_open.txt");
const CLOSE: &str = fixture!("tunnels/order_close.txt");
const FORGET: &str = fixture!("tunnels/order_forget.txt");
const HELD_LINE: &str = fixture!("tunnels/held_line.txt");
const VIEW_LINE: &str = fixture!("tunnels/view_line.json");
const TAKERS: &str = fixture!("tunnels/tunnels_takers.json");
const OLDER: &str = fixture!("tunnels/tunnels_older.json");

#[test]
fn the_first_line_says_the_window_takes_orders() {
    let Some(OrdersLine::Taking(list)) = orders_line(TAKING) else { panic!("a taking line") };
    assert_eq!((list.takers, list.windows, list.tunnels.len()), (1, 0, 0));
}

#[test]
fn the_first_line_lists_the_holds_already_there() {
    let Some(OrdersLine::Taking(list)) = orders_line(TAKING_HELD) else { panic!("a taking line") };
    let [seen] = list.tunnels.as_slice() else { panic!("one tunnel") };
    assert_eq!((seen.kind.as_str(), seen.port, seen.local), ("hold", 8765, Some(60785)));
}

#[test]
fn each_order_line_gives_its_port() {
    assert_eq!(orders_line(OPEN), Some(OrdersLine::Order(Order::Open(8765))));
    assert_eq!(orders_line(CLOSE), Some(OrdersLine::Order(Order::Close(8765))));
    assert_eq!(orders_line(FORGET), Some(OrdersLine::Order(Order::Forget(8765))));
}

#[test]
fn a_line_that_is_neither_is_no_order() {
    let lines = [
        HELD_LINE,
        VIEW_LINE,
        r#"{"order":"reboot","port":1}"#,
        r#"{"order":"open","port":0}"#,
        r#"{"order":"open"}"#,
        r#"{"port":5173"#,
        "garbage",
        "",
    ];
    for line in lines {
        assert_eq!(orders_line(line), None, "{line}");
    }
}

#[test]
fn an_older_servers_list_counts_no_taker() {
    let older: TunnelList = serde_json::from_str(OLDER).expect("older list");
    let newer: TunnelList = serde_json::from_str(TAKERS).expect("takers list");
    assert_eq!((older.takers, newer.takers), (0, 1));
}

#[test]
fn orders_run_with_the_windows_remote_flags() {
    let target = Target::Remote(Remote::new("devbox", "~/bin/cornercase").expect("a destination"));
    let command = target.command(&orders_args());
    let all: Vec<&OsStr> = command.get_args().collect();
    assert_eq!(all, ["--remote", "devbox", "--remote-command", "~/bin/cornercase", "tunnel", "orders"]);
}

#[test]
fn a_machines_orders_run_with_its_destination_only() {
    let target = Target::Remote(Remote::new("vps.invalid", "").expect("a destination"));
    let command = target.command(&orders_args());
    let all: Vec<&OsStr> = command.get_args().collect();
    assert_eq!(all, ["--remote", "vps.invalid", "tunnel", "orders"]);
}

#[test]
fn a_refusal_is_one_line_after_a_separator() {
    assert_eq!(failed_args(8765, "-x\n  second"), ["tunnel", "failed", "--", "8765", "-x second"]);
}
