use std::time::{Duration, Instant};

use super::{Card, ENDED, REASON_MOST, Seen, Step, UNTIED, WindowProcesses, shortened};
use crate::test_util::fixture;

const NOTES: &str = "acme.notes";
const AWAY: Option<&str> = Some("devbox");

fn lines() -> Vec<&'static str> {
    fixture!("plugins/windows.jsonl").lines().collect()
}

fn seen_after(count: usize, now: Instant) -> Seen {
    let mut seen = Seen::default();
    for line in &lines()[..count] {
        seen.read(line, now).expect("the line parses");
    }
    seen
}

fn described(seen: &Seen, plugin: &str, now: Instant) -> Vec<String> {
    match seen.card(plugin, now) {
        Card::Processes(services) => services.iter().map(super::Service::describe).collect(),
        other => panic!("no processes: {other:?}"),
    }
}

#[test]
fn a_running_line_gives_each_process_its_pid() {
    let now = Instant::now();
    let shown = described(&seen_after(2, now), NOTES, now);
    assert_eq!(shown, ["flaky running, pid 4210", "panel running, pid 4211"]);
}

#[test]
fn a_stopped_line_says_when_it_starts_again() {
    let now = Instant::now();
    let shown = described(&seen_after(3, now), NOTES, now);
    assert_eq!(shown, ["flaky stopped, restarting in 2 s", "panel running, pid 4211"]);
}

#[test]
fn the_seconds_count_down_from_when_the_line_came() {
    let now = Instant::now();
    let seen = seen_after(3, now);
    let later = |seconds| now + Duration::from_secs(seconds);
    assert_eq!(described(&seen, NOTES, later(1))[0], "flaky stopped, restarting in 1 s");
    assert_eq!(described(&seen, NOTES, later(9))[0], "flaky stopped, restarting now");
}

#[test]
fn a_new_pid_replaces_the_stop_before_it() {
    let now = Instant::now();
    let shown = described(&seen_after(4, now), NOTES, now);
    assert_eq!(shown, ["flaky running, pid 4329", "panel running, pid 4211"]);
}

#[test]
fn a_process_that_will_not_start_again_leaves_the_card() {
    let now = Instant::now();
    assert_eq!(described(&seen_after(5, now), NOTES, now), ["panel running, pid 4211"]);
    assert_eq!(seen_after(6, now).card(NOTES, now), Card::Starting);
}

#[test]
fn only_a_stop_with_a_time_counts_down() {
    let now = Instant::now();
    assert!(!seen_after(2, now).counting());
    assert!(seen_after(3, now).counting());
    assert!(!seen_after(4, now).counting());
}

#[test]
fn a_plugin_is_shown_only_its_own_processes() {
    let now = Instant::now();
    let mut seen = seen_after(2, now);
    let other = r#"{"plugin":"acme.deploy","id":"panel","state":"running","pid":77,"restart_in":null}"#;
    seen.read(other, now).expect("the line parses");
    assert_eq!(described(&seen, "acme.deploy", now), ["panel running, pid 77"]);
    assert_eq!(described(&seen, NOTES, now).len(), 2);
}

#[test]
fn a_state_added_later_is_shown_by_name() {
    let now = Instant::now();
    let mut seen = Seen::default();
    let line = r#"{"plugin":"acme.notes","id":"panel","state":"paused","pid":null,"restart_in":null}"#;
    seen.read(line, now).expect("the line parses");
    assert_eq!(described(&seen, NOTES, now), ["panel paused"]);
}

#[test]
fn before_its_first_line_a_plugin_is_starting() {
    let now = Instant::now();
    assert_eq!(Seen::default().card(NOTES, now), Card::Starting);
}

#[test]
fn a_line_that_cannot_be_read_changes_nothing() {
    let now = Instant::now();
    let mut seen = seen_after(2, now);
    seen.read("not json", now).expect_err("the failure");
    assert_eq!(described(&seen, NOTES, now).len(), 2);
}

#[test]
fn the_commands_end_takes_every_process_with_it_and_says_why() {
    let now = Instant::now();
    let mut seen = seen_after(2, now);
    let why = seen.end("first line\nit fell over\n");
    assert_eq!(why, "it fell over");
    assert_eq!(seen.card(NOTES, now), Card::NotRunning(why));
    assert!(!seen.counting());
}

#[test]
fn an_end_with_nothing_on_its_error_output_names_the_command() {
    assert_eq!(Seen::default().end(""), ENDED);
}

#[test]
fn a_window_on_this_computer_starts_them() {
    assert_eq!(WindowProcesses::default().step(None), Step::Start);
}

#[test]
fn a_window_on_this_computer_keeps_the_one_it_has() {
    let mut windows = WindowProcesses::default();
    windows.began();
    assert_eq!(windows.step(None), Step::Stay);
}

#[test]
fn a_window_opened_on_another_server_starts_nothing() {
    assert_eq!(WindowProcesses::default().step(AWAY), Step::Stay);
}

#[test]
fn moving_to_another_server_stops_them() {
    let mut windows = WindowProcesses::default();
    windows.began();
    assert_eq!(windows.step(AWAY), Step::Stop);
}

#[test]
fn coming_back_starts_them_again() {
    let mut windows = WindowProcesses::default();
    windows.began();
    windows.stopped();
    assert_eq!(windows.step(AWAY), Step::Stay);
    assert_eq!(windows.step(None), Step::Start);
}

#[test]
fn a_retry_due_while_away_starts_nothing() {
    let mut windows = WindowProcesses::default();
    let run = windows.began();
    windows.ended(run, "it fell over").expect("it fell");
    assert_eq!(windows.step(AWAY), Step::Stay);
}

#[test]
fn stopping_forgets_every_process_and_leaves_nothing_to_say() {
    let now = Instant::now();
    let mut windows = WindowProcesses::default();
    let run = windows.began();
    windows.read_line(run, lines()[0], now).expect("the line parses");
    windows.stopped();
    assert_eq!(windows.seen.card(NOTES, now), Card::Starting);
}

#[test]
fn the_end_of_a_run_that_was_stopped_says_nothing_and_tries_nothing() {
    let now = Instant::now();
    let mut windows = WindowProcesses::default();
    let run = windows.began();
    windows.stopped();
    assert!(windows.ended(run, "it ended").is_none());
    assert_eq!(windows.seen.card(NOTES, now), Card::Starting);
    assert_eq!(windows.step(AWAY), Step::Stay);
}

#[test]
fn the_end_of_a_stopped_run_leaves_the_next_run_kept() {
    let mut windows = WindowProcesses::default();
    let stopped = windows.began();
    windows.stopped();
    windows.began();
    assert!(windows.ended(stopped, "it ended").is_none());
    assert_eq!(windows.step(None), Step::Stay);
}

#[test]
fn a_line_of_a_run_that_was_stopped_is_dropped() {
    let now = Instant::now();
    let mut windows = WindowProcesses::default();
    let stopped = windows.began();
    windows.stopped();
    windows.began();
    windows.read_line(stopped, lines()[0], now).expect("dropped");
    assert_eq!(windows.seen.card(NOTES, now), Card::Starting);
}

#[test]
fn a_run_that_fell_is_told_once_and_waits_longer_each_time() {
    let mut windows = WindowProcesses::default();
    let run = windows.began();
    let first = windows.ended(run, "it fell over").expect("it fell");
    assert_eq!(first.say.as_deref(), Some("The plugins' window processes stopped: it fell over"));
    let run = windows.began();
    let second = windows.ended(run, "it fell over").expect("it fell");
    assert_eq!(second.say, None);
    assert!(second.wait > first.wait);
}

#[test]
fn a_line_between_two_falls_tells_the_next_one_again() {
    let now = Instant::now();
    let mut windows = WindowProcesses::default();
    let run = windows.began();
    windows.ended(run, "it fell over").expect("it fell");
    let run = windows.began();
    windows.read_line(run, lines()[0], now).expect("the line parses");
    assert!(windows.ended(run, "it fell over").expect("it fell").say.is_some());
}

#[test]
fn a_job_windows_refused_is_told_once_per_window() {
    let mut processes = WindowProcesses::default();
    let told = processes.untied(Some("Access is denied.".to_string())).expect("the first time it is told");
    assert!(told.starts_with(UNTIED));
    assert!(told.ends_with("Access is denied."));
    processes.stopped();
    assert_eq!(processes.untied(Some("Access is denied.".to_string())), None);
}

#[test]
fn a_tied_start_says_nothing_and_leaves_the_one_telling() {
    let mut processes = WindowProcesses::default();
    assert_eq!(processes.untied(None), None);
    assert!(processes.untied(Some("no".to_string())).is_some());
}

#[test]
fn a_reason_of_up_to_300_characters_is_shown_whole() {
    assert_eq!(shortened(&"x".repeat(REASON_MOST)), None);
    assert_eq!(shortened("permission denied"), None);
}

#[test]
fn a_longer_reason_ends_in_an_ellipsis_after_its_first_300_characters() {
    let cut = shortened(&"x".repeat(600)).expect("a long line is cut");
    assert_eq!(cut, format!("{}…", "x".repeat(300)));
}

#[test]
fn a_reason_is_cut_between_characters_not_bytes() {
    let cut = shortened(&"é".repeat(400)).expect("a long line is cut");
    assert_eq!(cut, format!("{}…", "é".repeat(300)));
}

#[test]
fn a_cut_reason_drops_the_space_before_its_ellipsis() {
    let cut = shortened(&"word ".repeat(100)).expect("a long line is cut");
    assert!(cut.ends_with("word…"));
}
