use std::io::{BufRead, BufReader, Read};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const OUTPUT_BUDGET: u64 = 256 * 1024;

pub(crate) fn lines_within(
    cmd: &mut Command,
    limit: Duration,
    done: fn(&[String]) -> bool,
) -> Result<Vec<String>, String> {
    let deadline = Instant::now() + limit;
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let (tx, rx) = mpsc::channel();
    if let Some(stdout) = child.stdout.take() {
        thread::spawn(move || forward(stdout.take(OUTPUT_BUDGET), &tx));
    }
    let lines = gather(&rx, deadline, done);
    let timed_out = Instant::now() >= deadline;
    let status = if timed_out { None } else { child.wait().ok() };
    let _ = child.kill();
    let _ = child.wait();
    silence(lines, status, limit)
}

fn silence(lines: Vec<String>, status: Option<ExitStatus>, limit: Duration) -> Result<Vec<String>, String> {
    if !lines.is_empty() {
        return Ok(lines);
    }
    match status {
        Some(status) if !status.success() => Err(format!("it exited with {status} and printed nothing")),
        Some(_) => Ok(lines),
        None => Err(format!("it did not answer within {} s", limit.as_secs())),
    }
}

fn forward(stdout: impl Read, tx: &mpsc::Sender<String>) {
    let lines = BufReader::new(stdout).lines().map_while(Result::ok);
    for line in lines {
        if tx.send(line).is_err() {
            return;
        }
    }
}

fn gather(rx: &mpsc::Receiver<String>, deadline: Instant, done: fn(&[String]) -> bool) -> Vec<String> {
    let mut lines = Vec::new();
    while !done(&lines) {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        match rx.recv_timeout(left) {
            Ok(line) => lines.push(line),
            Err(_) => break,
        }
    }
    lines
}

#[cfg(test)]
mod tests;
