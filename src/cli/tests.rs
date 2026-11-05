use std::ffi::OsStr;
use std::process::Command;
use std::sync::mpsc;

use serde_json::json;

use super::{BINARY, Binary, CALLER, Job, Remote, Target, gather_input, gather_scroll, spawn_failure, use_binary};

fn args(cmd: &Command) -> Vec<&str> {
    cmd.get_args().filter_map(OsStr::to_str).collect()
}

fn remote(program: &str) -> Target {
    Target::Remote(Remote::new("devbox", program).expect("a destination"))
}

#[test]
fn a_local_command_runs_as_it_is_given() {
    assert_eq!(args(&Target::Local.command(&["status", "--json"])), ["status", "--json"]);
}

#[test]
fn a_remote_command_asks_cornercase_to_run_it_there() {
    let cmd = remote("").command(&["input", "--follow", "--json", "-"]);
    assert_eq!(args(&cmd), ["--remote", "devbox", "input", "--follow", "--json", "-"]);
}

#[test]
fn a_remote_command_says_how_to_run_cornercase_there() {
    let cmd = remote("env HOME=/tmp/h cornercase").command(&["status"]);
    assert_eq!(args(&cmd), ["--remote", "devbox", "--remote-command", "env HOME=/tmp/h cornercase", "status"]);
}

#[test]
fn no_command_inherits_the_pane_or_the_remote_it_was_launched_from() {
    for target in [Target::Local, remote("")] {
        let cmd = target.command(&["status"]);
        let removed: Vec<&OsStr> = cmd.get_envs().filter(|(_, value)| value.is_none()).map(|(name, _)| name).collect();
        for name in CALLER {
            assert!(removed.contains(&OsStr::new(name)), "{name}");
        }
    }
}

#[test]
fn keys_for_several_panes_go_in_one_batch_in_order() {
    let (tx, rx) = mpsc::channel();
    tx.send(Job::Input { pane: 4, event: json!({ "text": "b" }) }).expect("queued");
    tx.send(Job::Input { pane: 3, event: json!({ "text": "c" }) }).expect("queued");
    let (events, pending) = gather_input(&rx, (3, json!({ "text": "a" })));
    let panes: Vec<u64> = events.iter().map(|(pane, _)| *pane).collect();
    assert_eq!(panes, [3, 4, 3]);
    assert!(pending.is_none());
}

#[test]
fn a_command_after_keys_ends_their_batch_and_runs_next() {
    let (tx, rx) = mpsc::channel();
    tx.send(Job::Input { pane: 3, event: json!({ "text": "b" }) }).expect("queued");
    super::Queue { tx: tx.clone() }.cmd(vec!["focus".into()]);
    tx.send(Job::Input { pane: 3, event: json!({ "text": "c" }) }).expect("queued");
    let (events, pending) = gather_input(&rx, (3, json!({ "text": "a" })));
    assert_eq!(events.len(), 2);
    assert!(matches!(pending, Some(Job::Cmd(cmd)) if cmd.args == ["focus"]));
}

#[test]
fn wheel_turns_on_one_pane_add_up_to_one_scroll() {
    let (tx, rx) = mpsc::channel();
    let queue = super::Queue { tx };
    queue.scroll(3, 4);
    queue.scroll(3, -1);
    queue.scroll(3, 5);
    let (rows, pending) = gather_scroll(&rx, 3, 2);
    assert_eq!(rows, 10);
    assert!(pending.is_none());
}

#[test]
fn a_scroll_of_another_pane_ends_the_sum_and_runs_next() {
    let (tx, rx) = mpsc::channel();
    let queue = super::Queue { tx };
    queue.scroll(4, 1);
    queue.scroll(3, 1);
    let (rows, pending) = gather_scroll(&rx, 3, 2);
    assert_eq!(rows, 2);
    assert!(matches!(pending, Some(Job::Scroll { pane: 4, rows: 1 })));
}

#[test]
fn a_reason_loses_the_command_name() {
    let e = anyhow::anyhow!("cornercase config: error: `fetch_minutes` takes a whole number");
    assert_eq!(super::reason(&e), "`fetch_minutes` takes a whole number");
    let e = anyhow::anyhow!("cornercase config: Error: the token was refused");
    assert_eq!(super::reason(&e), "the token was refused");
    assert_eq!(super::reason(&anyhow::anyhow!("could not run")), "could not run");
}

fn path_of(cmd: &Command) -> Option<&OsStr> {
    cmd.get_envs().find(|(name, _)| *name == OsStr::new("PATH")).and_then(|(_, value)| value)
}

#[test]
fn a_binary_found_through_the_login_shell_runs_with_that_path() {
    let binary =
        Binary { path: "/opt/homebrew/bin/cornercase".into(), path_env: Some("/opt/homebrew/bin:/usr/bin".into()) };
    let cmd = Target::Local.command_with(&binary, &["status"]);
    assert_eq!(cmd.get_program(), "/opt/homebrew/bin/cornercase");
    assert_eq!(path_of(&cmd), Some(OsStr::new("/opt/homebrew/bin:/usr/bin")));
}

#[test]
fn a_binary_found_on_the_path_leaves_the_environment_alone() {
    let cmd = Target::Local.command_with(&Binary::plain("/usr/bin/cornercase".into()), &["status"]);
    assert_eq!(path_of(&cmd), None);
}

#[test]
fn a_binary_that_cannot_start_is_named_and_forgotten_only_if_it_is_still_the_cached_one() {
    use_binary(Binary::plain("/opt/new/cornercase".into()));
    let old = spawn_failure(OsStr::new("/opt/old/cornercase"), std::io::Error::other("gone"));
    assert_eq!(old.to_string(), "could not run `/opt/old/cornercase`");
    assert!(BINARY.lock().is_some());
    let new = spawn_failure(OsStr::new("/opt/new/cornercase"), std::io::Error::other("gone"));
    assert_eq!(new.to_string(), "could not run `/opt/new/cornercase`");
    assert!(BINARY.lock().is_none());
}
