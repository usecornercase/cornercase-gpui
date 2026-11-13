use std::process::{Command, Stdio};

use anyhow::Result;

use super::Target;
use super::kept::{End, Kept};

static HOLDS: Kept = Kept::empty(End::TakeAlong);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Ended {
    pub(crate) code: Option<i32>,
    pub(crate) said: String,
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
    HOLDS.start(command, on_line, move |said, code| on_end(Ended { code, said: said.trim().to_string() }))
}

pub(crate) fn end_hold(id: u64) {
    HOLDS.stop_one(id);
}

pub(crate) fn end_holds() {
    HOLDS.stop();
}

#[cfg(test)]
mod tests;
