use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::gather;

fn closed(lines: &[String]) -> bool {
    lines.iter().filter(|line| line.trim() == "mark").count() >= 2
}

#[test]
fn gathering_stops_at_the_closing_mark() {
    let (tx, rx) = mpsc::channel();
    for line in ["noise", "mark", "/bin", "mark", "later"] {
        tx.send(line.to_string()).expect("the channel is open");
    }
    let lines = gather(&rx, Instant::now() + Duration::from_secs(60), closed);
    assert_eq!(lines.len(), 4);
}

#[test]
fn gathering_ends_with_the_channel() {
    let (tx, rx) = mpsc::channel();
    tx.send("one".to_string()).expect("the channel is open");
    drop(tx);
    assert_eq!(gather(&rx, Instant::now() + Duration::from_secs(60), closed), ["one"]);
}

#[cfg(unix)]
#[test]
fn a_program_that_exits_with_an_error_and_prints_nothing_says_so() {
    let mut cmd = crate::platform::command("/bin/sh");
    cmd.args(["-c", "exit 3"]);
    let answer = super::lines_within(&mut cmd, Duration::from_secs(30), |lines| !lines.is_empty());
    assert!(answer.expect_err("an error").contains("exit status: 3"));
}

#[test]
fn queued_lines_after_the_deadline_are_not_read() {
    let (tx, rx) = mpsc::channel();
    tx.send("late".to_string()).expect("the channel is open");
    assert_eq!(gather(&rx, Instant::now(), closed).len(), 0);
}
