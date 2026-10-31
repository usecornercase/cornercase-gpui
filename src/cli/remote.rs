use std::time::Duration;

use anyhow::{Context as _, Result};

const LONGEST_WAIT: u64 = 15;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Remote {
    destination: String,
    program: Option<String>,
}

impl Remote {
    pub(crate) fn new(destination: &str, program: &str) -> Option<Self> {
        let destination = destination.trim();
        if destination.is_empty() || destination.starts_with('-') || destination.contains(char::is_whitespace) {
            return None;
        }
        let program = program.trim();
        Some(Self { destination: destination.to_string(), program: (!program.is_empty()).then(|| program.to_string()) })
    }

    pub(super) fn from_args(args: &[String]) -> Result<Option<Self>> {
        let value =
            |flag: &str| args.iter().position(|a| a == flag).map(|at| args.get(at + 1).map_or("", String::as_str));
        let Some(destination) = value("--remote") else { return Ok(None) };
        let remote = Self::new(destination, value("--remote-command").unwrap_or_default());
        remote
            .map(Some)
            .with_context(|| format!("--remote needs a host from ~/.ssh/config or user@host, not {destination:?}"))
    }

    pub(crate) fn destination(&self) -> &str {
        &self.destination
    }

    pub(crate) fn program(&self) -> Option<&str> {
        self.program.as_deref()
    }

    pub(super) fn flags(&self) -> Vec<String> {
        let mut flags = vec!["--remote".to_string(), self.destination.clone()];
        if let Some(program) = &self.program {
            flags.extend(["--remote-command".to_string(), program.clone()]);
        }
        flags
    }
}

pub(crate) fn backoff(attempts: u32) -> Duration {
    Duration::from_secs(1u64.checked_shl(attempts).unwrap_or(LONGEST_WAIT).min(LONGEST_WAIT))
}

pub(crate) fn from_home(path: &str) -> String {
    let path = path.trim();
    let relative = if path == "~" { "" } else { path.strip_prefix("~/").unwrap_or(path) };
    if relative.is_empty() { ".".to_string() } else { relative.to_string() }
}

#[cfg(test)]
mod tests;
