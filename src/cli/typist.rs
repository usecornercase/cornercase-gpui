use std::io::{BufRead as _, BufReader, Write as _};
use std::process::{ChildStdin, ChildStdout};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

use serde_json::Value;

use super::{Piped, Stream, Target, piped, read_all};

const SETTLE: Duration = Duration::from_secs(10);

pub(super) type OnError = Arc<dyn Fn(String) + Send + Sync>;

pub(super) struct Typist {
    stdin: ChildStdin,
    answered: mpsc::Receiver<usize>,
    unanswered: usize,
    stopping: Arc<AtomicBool>,
    _process: Stream,
}

impl Typist {
    pub(super) fn start(target: &Target, on_error: &OnError) -> Option<Self> {
        let Piped { stdin, stdout, stderr, process } = piped(target, &["input", "--follow", "--json", "-"])?;
        let (tx, answered) = mpsc::channel();
        let stopping = Arc::new(AtomicBool::new(false));
        let (on_error, stopped) = (on_error.clone(), stopping.clone());
        thread::spawn(move || {
            let errors = thread::spawn(move || read_all(stderr));
            read_answers(stdout, &tx, &*on_error);
            let why = errors.join().unwrap_or_default();
            if !stopped.load(Ordering::Relaxed) {
                on_error(ended(&why));
            }
        });
        Some(Self { stdin, answered, unanswered: 0, stopping, _process: process })
    }

    pub(super) fn send(&mut self, lines: &[String]) -> bool {
        let text = lines.join("\n") + "\n";
        let sent = self.stdin.write_all(text.as_bytes()).and_then(|()| self.stdin.flush()).is_ok();
        if sent {
            self.unanswered += lines.len();
        }
        sent
    }

    pub(super) fn settle(&mut self) -> bool {
        while self.unanswered > 0 {
            match self.answered.recv_timeout(SETTLE) {
                Ok(events) => self.unanswered = self.unanswered.saturating_sub(events),
                Err(_) => return false,
            }
        }
        true
    }
}

impl Drop for Typist {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Relaxed);
    }
}

fn read_answers(stdout: ChildStdout, answered: &mpsc::Sender<usize>, on_error: &(dyn Fn(String) + Send + Sync)) {
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        let (events, error) = answer(&line);
        if let Some(error) = error {
            on_error(format!("cornercase input: {error}"));
        }
        if answered.send(events).is_err() {
            return;
        }
    }
}

pub(super) fn line(pane: u64, event: &Value) -> String {
    let mut event = event.clone();
    if let Some(fields) = event.as_object_mut() {
        fields.insert("pane".to_string(), pane.into());
    }
    event.to_string()
}

fn answer(line: &str) -> (usize, Option<String>) {
    let Ok(value) = serde_json::from_str::<Value>(line) else { return (1, Some(line.trim().to_string())) };
    let events = value.get("events").and_then(Value::as_u64).and_then(|n| usize::try_from(n).ok()).unwrap_or(1);
    (events, value.get("error").and_then(Value::as_str).map(str::to_string))
}

fn ended(why: &str) -> String {
    let why = why.lines().map(str::trim).rfind(|l| !l.is_empty()).unwrap_or("it ended");
    format!("cornercase input --follow: {why}")
}

#[cfg(test)]
mod tests;
