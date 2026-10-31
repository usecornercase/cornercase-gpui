use super::{REVERSE_PORTS, forward_args, reverse_args};

fn has(args: &[String], pair: [&str; 2]) -> bool {
    args.windows(2).any(|w| w[0] == pair[0] && w[1] == pair[1])
}

#[test]
fn a_forward_binds_this_mac_only_and_reaches_localhost_there() {
    let args = forward_args("devbox", 61234, 3000);
    assert!(has(&args, ["-L", "127.0.0.1:61234:localhost:3000"]));
    assert_eq!(args[args.len() - 2..], ["--", "devbox"]);
}

#[test]
fn a_tunnel_runs_no_command_asks_nothing_and_fails_when_it_cannot_forward() {
    let args = forward_args("devbox", 3000, 3000);
    assert!(args.contains(&"-N".to_string()));
    assert!(has(&args, ["-o", "BatchMode=yes"]));
    assert!(has(&args, ["-o", "ExitOnForwardFailure=yes"]));
    assert!(has(&args, ["-o", "RemoteCommand=none"]));
}

#[test]
fn a_tunnel_never_shares_a_master_connection() {
    let args = reverse_args("devbox", (9222, 9222), 50000);
    assert!(has(&args, ["-o", "ControlMaster=no"]));
    assert!(has(&args, ["-o", "ControlPath=none"]));
}

#[test]
fn the_reverse_tunnel_brings_the_browser_there_and_checks_it_through_itself() {
    let args = reverse_args("devbox", (9223, 9222), 50000);
    assert!(has(&args, ["-R", "127.0.0.1:9223:127.0.0.1:9222"]));
    assert!(has(&args, ["-L", "127.0.0.1:50000:127.0.0.1:9223"]));
}

#[test]
fn agents_there_are_offered_the_usual_devtools_port_first() {
    assert_eq!(REVERSE_PORTS[0], 9222);
}
