use crate::cli;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Asked {
    Click,
    Open,
    Back,
    Retry,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Held {
    Opening,
    Up { local: u16 },
    Down { why: String },
    Ending,
}

#[derive(Debug, Clone)]
pub(crate) struct Hold {
    pub(crate) port: u16,
    pub(crate) process: u64,
    pub(crate) state: Held,
    pub(crate) asked: Asked,
    pub(crate) was_up: bool,
    pub(crate) boot: String,
    pub(crate) attempts: u32,
    pub(crate) job: Option<u64>,
    pub(crate) last_local: Option<u16>,
}

impl Hold {
    pub(crate) fn opening(port: u16, process: u64, asked: Asked, boot: &str) -> Self {
        Self {
            port,
            process,
            state: Held::Opening,
            asked,
            was_up: false,
            boot: boot.to_string(),
            attempts: 0,
            job: None,
            last_local: None,
        }
    }

    pub(crate) fn local(&self) -> Option<u16> {
        match self.state {
            Held::Up { local } => Some(local),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Fate {
    Gone,
    Failed(String),
    Retry(String),
    Settle,
}

pub(crate) fn fate(hold: &Hold, code: Option<i32>, said: &str) -> Fate {
    if hold.state == Held::Ending {
        return Fate::Gone;
    }
    if code == Some(0) {
        return Fate::Settle;
    }
    let why = said_line(said, code);
    if hold.was_up { Fate::Retry(why) } else { Fate::Failed(why) }
}

fn said_line(said: &str, code: Option<i32>) -> String {
    let line = cli::reason_of(said);
    if !line.is_empty() {
        return capitalised(&line);
    }
    match code {
        Some(code) => format!("cornercase tunnel hold ended with status {code}"),
        None => "cornercase tunnel hold was stopped".to_string(),
    }
}

fn capitalised(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
}

pub(crate) fn failed_toast(port: u16, why: &str) -> String {
    if why.starts_with("Could not tunnel port") {
        why.to_string()
    } else {
        format!("Could not tunnel port {port}: {why}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Settled {
    Closed,
    Back,
}

pub(crate) fn settled(started_on: &str, now: &str) -> Settled {
    if started_on == now { Settled::Closed } else { Settled::Back }
}

pub(crate) fn bring_back(remembered: &[u16], held: &[u16]) -> Vec<u16> {
    remembered.iter().copied().filter(|port| !held.contains(port)).collect()
}

pub(crate) fn up_toast(asked: Asked, port: u16, local: u16, host: &str) -> Option<(String, bool)> {
    let moved = local != port;
    match asked {
        Asked::Click => Some((tunnelled(port, local, host, moved), true)),
        Asked::Open if moved => Some((tunnelled(port, local, host, true), false)),
        Asked::Open | Asked::Back | Asked::Retry => None,
    }
}

fn tunnelled(port: u16, local: u16, host: &str, moved: bool) -> String {
    let taken = if moved { format!(" ({port} is taken on this {})", crate::desktop::here()) } else { String::new() };
    format!("Port {port} on {host} is tunnelled to localhost:{local}{taken}")
}

#[cfg(test)]
mod tests;
