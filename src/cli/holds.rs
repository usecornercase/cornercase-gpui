use std::process::{Command, Stdio};
use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;

use super::Target;
use super::kept::{End, Kept};

static HOLDS: Kept = Kept::empty(End::TakeAlong);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Ended {
    pub(crate) code: Option<i32>,
    pub(crate) said: String,
    pub(crate) last_line: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Holding {
    Forward { port: u16, local: Option<u16> },
    Reverse { local: u16 },
}

impl Holding {
    fn args(self) -> Vec<String> {
        let mut all = vec!["tunnel".to_string(), "hold".to_string()];
        match self {
            Self::Forward { port, local } => {
                all.push(port.to_string());
                if let Some(local) = local {
                    all.extend(["--local".to_string(), local.to_string()]);
                }
            }
            Self::Reverse { local } => all.extend(["--reverse".to_string(), local.to_string()]),
        }
        all
    }
}

pub(crate) fn hold(
    target: &Target,
    holding: Holding,
    on_line: impl FnMut(String) + Send + 'static,
    on_end: impl FnOnce(Ended) + Send + 'static,
) -> Result<(u64, Option<String>)> {
    hold_with(target.command(&holding.args()), on_line, on_end)
}

fn hold_with(
    mut command: Command,
    on_line: impl FnMut(String) + Send + 'static,
    on_end: impl FnOnce(Ended) + Send + 'static,
) -> Result<(u64, Option<String>)> {
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let printed = Arc::new(Mutex::new(String::new()));
    let kept = Arc::clone(&printed);
    let mut on_line = on_line;
    let keeping = move |line: String| {
        keep_last(&mut kept.lock(), &line);
        on_line(line);
    };
    HOLDS.start(command, keeping, move |said, code| {
        let last_line = std::mem::take(&mut *printed.lock());
        on_end(Ended { code, said: said.trim().to_string(), last_line });
    })
}

fn keep_last(last: &mut String, line: &str) {
    let line = line.trim();
    if !line.is_empty() {
        line.clone_into(last);
    }
}

#[cfg(test)]
pub(crate) fn ended_after(printed: &str, code: Option<i32>) -> Ended {
    let mut last_line = String::new();
    for line in printed.lines() {
        keep_last(&mut last_line, line);
    }
    Ended { code, said: String::new(), last_line }
}

pub(crate) fn end_hold(id: u64) {
    HOLDS.stop_one(id);
}

pub(crate) fn end_holds() {
    HOLDS.stop();
}

#[cfg(test)]
mod tests;
