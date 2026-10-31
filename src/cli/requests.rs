use std::io::{BufRead as _, BufReader, Write as _};
use std::process::{ChildStdin, ChildStdout};
use std::sync::mpsc::{self, TryRecvError};
use std::thread;

use anyhow::{Result, anyhow, bail};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Piped, Stream, Target, piped, read_all};

pub(super) type Answered = std::result::Result<String, String>;

pub(super) struct Requests {
    stdin: ChildStdin,
    answers: mpsc::Receiver<Answered>,
    _process: Stream,
}

impl Requests {
    pub(super) fn start(target: &Target) -> Option<Self> {
        let Piped { stdin, stdout, stderr, process } = piped(target, &["requests", "--follow", "-"])?;
        let (tx, answers) = mpsc::channel();
        thread::spawn(move || {
            let errors = thread::spawn(move || read_all(stderr));
            read_answers(stdout, &tx);
            let _ = tx.send(Err(errors.join().unwrap_or_default()));
        });
        Some(Self { stdin, answers, _process: process })
    }

    pub(super) fn ask(&mut self, line: &str) -> Option<Answered> {
        if matches!(self.answers.try_recv(), Ok(Err(_)) | Err(TryRecvError::Disconnected)) {
            return None;
        }
        writeln!(self.stdin, "{line}").and_then(|()| self.stdin.flush()).ok()?;
        Some(self.answers.recv().unwrap_or_else(|_| Err(String::new())))
    }
}

fn read_answers(stdout: ChildStdout, answers: &mpsc::Sender<Answered>) {
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        if answers.send(Ok(line)).is_err() {
            return;
        }
    }
}

pub(super) fn line(args: &[String], stdin: Option<&[u8]>) -> Option<String> {
    let mut line = json!({ "args": args });
    if let Some(bytes) = stdin {
        line["stdin"] = Value::from(std::str::from_utf8(bytes).ok()?);
    }
    Some(line.to_string())
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Answer {
    status: i32,
    json: Option<Value>,
    text: String,
    stderr: String,
    error: String,
}

pub(super) fn answer(name: &str, line: &str) -> Result<String> {
    let Ok(answer) = serde_json::from_str::<Answer>(line) else { bail!("cornercase {name}: {}", line.trim()) };
    if answer.status == 0 {
        return Ok(printed(answer));
    }
    let why = [answer.error, answer.stderr].into_iter().map(|text| text.trim().to_string()).find(|t| !t.is_empty());
    bail!("cornercase {name}: {}", why.as_deref().unwrap_or("failed"))
}

fn printed(answer: Answer) -> String {
    match answer.json {
        Some(json) if answer.text.is_empty() => json.to_string(),
        Some(json) => format!("{json}\n{}", answer.text),
        None => answer.text,
    }
}

pub(super) fn lost(name: &str, why: &str) -> anyhow::Error {
    let why = why.lines().map(str::trim).rfind(|l| !l.is_empty()).unwrap_or("cornercase requests ended");
    anyhow!("cornercase {name}: {why}")
}

#[cfg(test)]
mod tests;
