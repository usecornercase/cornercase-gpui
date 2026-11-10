use std::ffi::OsStr;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};

use serde_json::json;

use super::{
    BINARY, Binary, CALLER, Job, Queue, QueueTap, Remote, Target, Worker, failure, gather_input, gather_scroll,
    moves_view, spawn_failure, stale_boot, use_binary,
};

fn args(cmd: &Command) -> Vec<&str> {
    cmd.get_args().filter_map(OsStr::to_str).collect()
}

fn queue(tx: mpsc::Sender<Job>) -> Queue {
    Queue { tx, view_moves: Arc::new(|| {}) }
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
    queue(tx.clone()).cmd(vec!["focus".into()]);
    tx.send(Job::Input { pane: 3, event: json!({ "text": "c" }) }).expect("queued");
    let (events, pending) = gather_input(&rx, (3, json!({ "text": "a" })));
    assert_eq!(events.len(), 2);
    assert!(matches!(pending, Some(Job::Cmd(cmd)) if cmd.args == ["focus"]));
}

#[test]
fn wheel_turns_on_one_pane_add_up_to_one_scroll() {
    let (tx, rx) = mpsc::channel();
    let queue = queue(tx);
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
    let queue = queue(tx);
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

#[test]
fn a_command_on_a_machine_asks_its_destination_and_not_the_windows_server() {
    let machine = Target::Remote(Remote::new("ana@vps.example.com", "").expect("a destination"));
    let cmd = machine.command_with(&Binary::plain("cornercase".into()), &["close", "--workspace", "7"]);
    let args: Vec<_> = cmd.get_args().map(|arg| arg.to_string_lossy().into_owned()).collect();
    assert_eq!(args, ["--remote", "ana@vps.example.com", "close", "--workspace", "7"]);
}

#[test]
fn what_shows_a_place_moves_the_view() {
    assert!(moves_view(&["focus", "--tab", "3"]));
    assert!(moves_view(&["go", "cc-1"]));
    assert!(moves_view(&["new-tab", "--workspace", "2", "--focus"]));
    assert!(moves_view(&["open", "/p", "--focus", "--json"]));
}

#[test]
fn what_only_changes_things_does_not_move_the_view() {
    assert!(!moves_view(&["rename", "--workspace", "2", "focus"]));
    assert!(!moves_view(&["collapse", "--workspace", "2"]));
    assert!(!moves_view::<&str>(&[]));
}

#[test]
fn a_command_through_the_queue_that_moves_the_view_tells_the_window() {
    let (tx, _rx) = mpsc::channel();
    let told = Arc::new(AtomicUsize::new(0));
    let counted = told.clone();
    let view_moves = Arc::new(move || {
        counted.fetch_add(1, Ordering::Relaxed);
    });
    let queue = Queue { tx, view_moves };
    queue.cmd(vec!["rename".into(), "--workspace".into(), "2".into(), "x".into()]);
    assert_eq!(told.load(Ordering::Relaxed), 0);
    queue.cmd(vec!["focus".into(), "--tab".into(), "3".into()]);
    queue.cmd_then(vec!["new-tab".into(), "--focus".into()], |_| {});
    assert_eq!(told.load(Ordering::Relaxed), 2);
}

#[test]
fn a_queue_made_for_a_machine_keeps_that_machine() {
    let worker = Worker::new(Some(remote("")), Arc::new(|_| {}));
    assert_eq!(worker.target, remote(""));
}

#[test]
fn a_queue_going_nowhere_takes_everything_and_sends_nothing() {
    let queue = Queue::nowhere();
    queue.input(3, json!({ "text": "a" }));
    queue.cmd(vec!["focus".into(), "--tab".into(), "3".into()]);
    queue.scroll(3, 1);
}

#[test]
fn what_a_machines_queue_is_asked_to_show_does_not_leave_its_place() {
    let (queue, tap) = QueueTap::new();
    queue.cmd(vec!["focus".into(), "--tab".into(), "3".into()]);
    assert_eq!(tap.commands(), [["focus", "--tab", "3"]]);
}

#[test]
fn a_command_ending_with_status_four_was_refused_for_its_boot() {
    let stale = failure("close", Some(4), b"error: this request is for the cornercase server started as 1-2\n");
    assert!(stale_boot(&stale));
    assert_eq!(super::reason(&stale), "this request is for the cornercase server started as 1-2");
}

#[test]
fn a_command_failing_any_other_way_was_not_refused_for_its_boot() {
    assert!(!stale_boot(&failure("close", Some(1), b"error: no such workspace")));
    assert!(!stale_boot(&failure("close", None, b"")));
    assert_eq!(super::reason(&failure("close", Some(1), b"")), "failed");
}

#[test]
fn a_reason_reads_the_same_from_a_text() {
    assert_eq!(super::reason_of("cornercase start-server: Error: x"), "x");
    assert_eq!(super::reason_of("error: y"), "y");
}
