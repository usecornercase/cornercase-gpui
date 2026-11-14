use std::ffi::OsStr;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};

use serde_json::json;

use super::{
    BINARY, Binary, CALLER, Job, Queue, QueueTap, Remote, StreamEnd, Target, Worker, answered, failure, gather_input,
    gather_scroll, moves_view, older_refusal, refused_boot, reported, spawn_failure, stale_boot, use_binary, with_boot,
};

fn args(cmd: &Command) -> Vec<&str> {
    cmd.get_args().filter_map(OsStr::to_str).collect()
}

fn queue(tx: mpsc::Sender<Job>) -> Queue {
    Queue { tx, view_moves: Arc::new(|| {}), run: None }
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
    let queue = Queue { tx, view_moves, run: None };
    queue.cmd(vec!["rename".into(), "--workspace".into(), "2".into(), "x".into()]);
    assert_eq!(told.load(Ordering::Relaxed), 0);
    queue.cmd(vec!["focus".into(), "--tab".into(), "3".into()]);
    queue.cmd_then(vec!["new-tab".into(), "--focus".into()], |_| {});
    assert_eq!(told.load(Ordering::Relaxed), 2);
}

#[test]
fn a_queue_made_for_a_machine_keeps_that_machine() {
    let worker = Worker::new(Some(remote("")), None, Arc::new(|_| {}));
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

#[test]
fn a_one_shot_answers_with_its_stdout_when_it_exits_0() {
    assert_eq!(answered("status", Some(0), b"{}", b"").expect("answered"), "{}");
}

#[test]
fn a_one_shot_refused_as_older_fails_with_the_refusal_and_any_other_failure_without() {
    let line = crate::test_util::fixture!("remote/older_machine.json").as_bytes();
    let older = answered("status", Some(5), line, b"error: older").expect_err("refused");
    assert!(older_refusal(&older).is_some());
    let other = answered("status", Some(1), line, b"error: other").expect_err("failed");
    assert!(older_refusal(&other).is_none());
    assert_eq!(other.to_string(), "cornercase status: error: other");
    assert!(older_refusal(&answered("status", None, b"", b"").expect_err("killed")).is_none());
}

fn words(line: &[&str]) -> Vec<String> {
    line.iter().map(ToString::to_string).collect()
}

fn queue_of(boot: Option<&str>) -> Worker {
    Worker::new(Some(remote("")), boot.map(str::to_string), Arc::new(|_| {}))
}

#[test]
fn a_boot_goes_at_the_end_of_a_command() {
    let args = with_boot(words(&["mark", "--pane", "9"]), "b1");
    assert_eq!(args, ["mark", "--pane", "9", "--boot", "b1"]);
}

#[test]
fn a_boot_goes_before_the_words_after_two_dashes() {
    let args = with_boot(words(&["account", "restart", "--json", "--pane", "9", "--", "claude", "work"]), "b1");
    assert_eq!(args, ["account", "restart", "--json", "--pane", "9", "--boot", "b1", "--", "claude", "work"]);
}

#[test]
fn a_command_that_already_names_its_run_gets_no_second_boot() {
    let once = with_boot(words(&["mark", "--pane", "9"]), "b1");
    assert_eq!(with_boot(once, "b1"), ["mark", "--pane", "9", "--boot", "b1"]);
    let text = with_boot(words(&["send", "--pane", "9", "--", "--boot"]), "b1");
    assert_eq!(text, ["send", "--pane", "9", "--boot", "b1", "--", "--boot"]);
}

#[test]
fn a_machines_queue_writes_its_runs_boot_into_every_command_it_sends() {
    let worker = queue_of(Some("b1"));
    let lines: [&[&str]; 10] = [
        &["scroll", "--pane", "3", "--up", "2"],
        &["split", "--pane", "3", "--focus", "--view", "5"],
        &["focus", "--tab", "4", "--view", "5"],
        &["ratio", "--tab", "4", "0.4"],
        &["selection", "--pane", "3", "--from", "0,0", "--to", "4,0"],
        &["view-set", "--view", "5", "--cols", "80", "--rows", "24"],
        &["redraw", "--surface", "6"],
        &["held", "--pane", "3", "--send-now"],
        &["paste-file", "--pane", "3", "/tmp/a.png"],
        &["paste-file", "--pane", "3", "--name", "pasted.png", "-"],
    ];
    for line in lines {
        let sent = worker.on_run(words(line));
        let at = sent.iter().position(|arg| arg == "--boot").expect(line[0]);
        assert_eq!(sent[at + 1], "b1", "{line:?}");
        assert_eq!(sent.len(), line.len() + 2, "{line:?}");
    }
}

#[test]
fn a_machines_queue_gives_its_colours_without_the_run() {
    assert_eq!(queue_of(Some("b1")).on_run(words(&["theme", "-"])), ["theme", "-"]);
}

#[test]
fn the_windows_own_queue_sends_its_commands_as_given() {
    let line = words(&["scroll", "--pane", "3", "--up", "2"]);
    assert_eq!(queue_of(None).on_run(line.clone()), line);
}

#[test]
fn a_command_of_a_machines_tab_that_names_its_run_already_gets_no_second_boot() {
    let close = with_boot(words(&["close", "--pane", "5"]), "b1");
    assert_eq!(queue_of(Some("b1")).on_run(close), ["close", "--pane", "5", "--boot", "b1"]);
}

#[test]
fn a_machines_one_shot_input_names_its_run() {
    assert_eq!(queue_of(Some("b1")).one_shot_input(3), ["input", "--pane", "3", "-", "--boot", "b1"]);
    assert_eq!(queue_of(None).one_shot_input(3), ["input", "--pane", "3", "-"]);
}

#[test]
fn a_machines_typist_is_started_with_its_runs_boot() {
    assert_eq!(queue_of(Some("b1")).typist_args(), ["input", "--follow", "--json", "-", "--boot", "b1"]);
    assert_eq!(queue_of(None).typist_args(), ["input", "--follow", "--json", "-"]);
}

#[test]
fn what_a_machines_queue_asks_requests_to_run_names_its_run() {
    let worker = queue_of(Some("b1"));
    for line in [super::scroll_args(3, -2), words(&["split", "--pane", "3", "--focus", "--view", "5"])] {
        let (args, sent) = worker.sent(&line, None);
        assert_eq!(args.iter().filter(|arg| *arg == "--boot").count(), 1, "{line:?}");
        let sent: serde_json::Value = serde_json::from_str(&sent.expect("a line")).expect("json");
        assert_eq!(sent["args"], json!(args), "{line:?}");
        assert!(args.windows(2).any(|pair| pair == ["--boot", "b1"]), "{line:?}");
    }
}

#[test]
fn what_the_windows_own_queue_asks_requests_to_run_is_as_given() {
    let line = super::scroll_args(3, -2);
    let (args, sent) = queue_of(None).sent(&line, None);
    assert_eq!(args, line);
    assert_eq!(sent.expect("a line"), json!({ "args": line }).to_string());
}

#[test]
fn a_queue_reports_what_it_fails_at_but_not_a_refused_run() {
    let heard = Arc::new(parking_lot::Mutex::new(Vec::new()));
    let sink = heard.clone();
    let worker = Worker::new(Some(remote("")), Some("b1".into()), Arc::new(move |said| sink.lock().push(said)));
    worker.report(&failure("scroll", Some(4), b"error: another run\n"));
    assert!(heard.lock().is_empty());
    worker.report(&failure("scroll", Some(1), b"there is no pane 3"));
    assert_eq!(*heard.lock(), ["cornercase scroll: there is no pane 3"]);
}

#[test]
fn a_command_nobody_waits_for_refused_for_its_run_is_not_reported() {
    let stale = failure("scroll", Some(4), b"error: this request is for the cornercase server started as 1-2\n");
    assert_eq!(reported(&stale), None);
}

#[test]
fn any_other_failure_of_a_command_nobody_waits_for_is_reported() {
    let other = failure("scroll", Some(1), b"there is no pane 3");
    assert_eq!(reported(&other).as_deref(), Some("cornercase scroll: there is no pane 3"));
}

#[test]
fn a_queue_made_for_a_run_keeps_the_run_and_hands_its_worker_the_boot() {
    let made = |run| Queue::made(Some(remote("")), run, Arc::new(|_| {}), Arc::new(|| {}));
    let (queue, worker, _rx) = made(Some(("vps".to_string(), Some("b1".to_string()))));
    assert_eq!((queue.machine(), queue.boot(), worker.boot.as_deref()), (Some("vps"), Some("b1"), Some("b1")));
    let (queue, worker, _rx) = made(Some(("vps".to_string(), None)));
    assert_eq!((queue.machine(), queue.boot(), worker.boot), (Some("vps"), None, None));
    let (queue, worker, _rx) = made(None);
    assert_eq!((queue.machine(), worker.boot), (None, None));
}

#[test]
fn a_stream_that_ended_with_status_4_was_refused_for_its_boot() {
    assert!(StreamEnd::new("error: another run".into(), Some(4)).stale_boot());
}

#[test]
fn a_stream_that_ended_otherwise_or_was_stopped_was_not() {
    for status in [Some(1), Some(0), None] {
        assert!(!StreamEnd::new(String::new(), status).stale_boot());
    }
}

#[test]
fn only_status_4_is_a_refused_boot_for_an_answer_read_by_hand() {
    let refused = refused_boot("files", Some(4), b"error: another run").expect("refused");
    assert!(stale_boot(&refused));
    assert!(refused_boot("files", Some(1), b"error").is_none());
    assert!(refused_boot("files", None, b"").is_none());
}
