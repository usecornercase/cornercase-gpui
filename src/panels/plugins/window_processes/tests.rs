use std::time::{Duration, Instant};

use super::{Card, ENDED, Seen};
use crate::test_util::fixture;

const NOTES: &str = "acme.notes";

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
