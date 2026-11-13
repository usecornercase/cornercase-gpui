use crate::cli::{self, Target};
use crate::model::machines::hide_destination;
use crate::model::tunnels::Toward;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Asked {
    Click,
    Open,
    Program,
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
pub(crate) struct Reached {
    pub(crate) toward: Toward,
    pub(crate) target: Target,
    pub(crate) boot: String,
    pub(crate) label: String,
    pub(crate) destination: String,
}

#[derive(Debug, Clone)]
pub(crate) struct Hold {
    pub(crate) toward: Toward,
    pub(crate) destination: String,
    pub(crate) port: u16,
    pub(crate) process: u64,
    pub(crate) state: Held,
    pub(crate) asked: Asked,
    pub(crate) was_up: bool,
    pub(crate) boot: String,
    pub(crate) attempts: u32,
    pub(crate) job: Option<u64>,
    pub(crate) last_local: Option<u16>,
    pub(crate) had_local: Option<u16>,
    pub(crate) added: bool,
}

impl Hold {
    pub(crate) fn opening(reached: &Reached, port: u16, process: u64, asked: Asked) -> Self {
        Self {
            toward: reached.toward.clone(),
            destination: reached.destination.clone(),
            port,
            process,
            state: Held::Opening,
            asked,
            was_up: false,
            boot: reached.boot.clone(),
            attempts: 0,
            job: None,
            last_local: None,
            had_local: None,
            added: false,
        }
    }

    pub(crate) fn label(&self) -> &str {
        match &self.toward {
            Toward::Server => &self.destination,
            Toward::Machine(name) => name,
        }
    }

    pub(crate) fn local(&self) -> Option<u16> {
        match self.state {
            Held::Up { local } => Some(local),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Again {
    pub(crate) attempts: u32,
    pub(crate) local: Option<u16>,
}

pub(crate) fn again_of(hold: &Hold) -> Again {
    Again { attempts: hold.attempts, local: hold.last_local.or(hold.had_local) }
}

pub(crate) fn local_before(again: Option<Again>, remembered: Option<u16>) -> Option<u16> {
    again.and_then(|again| again.local).or(remembered)
}

pub(crate) fn live<'a>(holds: &'a [Hold], toward: &Toward, port: u16) -> Option<&'a Hold> {
    let ours = || holds.iter().filter(move |hold| hold.port == port && hold.toward == *toward);
    ours().find(|hold| hold.state != Held::Ending).or_else(|| ours().next())
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
    let line = cli::reason_of(said);
    if hold.was_up {
        let said = match &hold.toward {
            Toward::Machine(name) if !line.is_empty() => hide_destination(&line, name, &hold.destination),
            _ => line,
        };
        return Fate::Retry(if said.is_empty() { status_words(code) } else { capitalised(&said) });
    }
    if line.is_empty() {
        return Fate::Failed(window_failure(&hold.toward, hold.port, &status_words(code)));
    }
    Fate::Failed(match &hold.toward {
        Toward::Server => capitalised(&line),
        Toward::Machine(name) => {
            window_failure(&hold.toward, hold.port, &machine_reason(&line, name, &hold.destination))
        }
    })
}

fn machine_reason(line: &str, name: &str, destination: &str) -> String {
    let hidden = hide_destination(line, name, destination);
    let lead = ["could not tunnel port ", "Could not tunnel port "];
    let rest = lead.iter().find_map(|lead| hidden.strip_prefix(lead)).and_then(|rest| rest.split_once(": "));
    rest.map_or_else(|| hidden.clone(), |(_, why)| why.to_string())
}

pub(super) fn status_words(code: Option<i32>) -> String {
    match code {
        Some(code) => format!("cornercase tunnel hold ended with status {code}"),
        None => "cornercase tunnel hold was stopped".to_string(),
    }
}

fn capitalised(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
}

pub(crate) fn window_failure(toward: &Toward, port: u16, why: &str) -> String {
    match toward {
        Toward::Server => format!("Could not tunnel port {port}: {why}"),
        Toward::Machine(name) => format!("Could not tunnel port {port} on {name}: {why}"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Settled {
    Superseded,
    Closed,
    Back,
}

pub(crate) fn settled(started_on: &str, now: &str) -> Settled {
    if started_on == now { Settled::Closed } else { Settled::Back }
}

pub(crate) fn forgets_after_close(live_hold: bool) -> bool {
    !live_hold
}

pub(crate) fn settle_now(
    (linked, live, waited): (bool, bool, bool),
    started_on: &str,
    now: Option<&str>,
) -> Option<Settled> {
    let now = now?;
    if !linked {
        return None;
    }
    if live {
        return Some(Settled::Superseded);
    }
    let decided = settled(started_on, now);
    (decided == Settled::Back || waited).then_some(decided)
}

pub(crate) fn ports_kept(holds: &[Hold], toward: &Toward, settling: impl Iterator<Item = u16>) -> Vec<u16> {
    holds.iter().filter(|hold| hold.toward == *toward).map(|hold| hold.port).chain(settling).collect()
}

pub(crate) fn bring_back(remembered: &[u16], held: &[u16]) -> Vec<u16> {
    remembered.iter().copied().filter(|port| !held.contains(port)).collect()
}

pub(crate) fn remembers(asked: Asked) -> bool {
    matches!(asked, Asked::Click | Asked::Open | Asked::Program)
}

pub(crate) fn forgets_failed(fate: &Fate, added: bool) -> bool {
    matches!(fate, Fate::Failed(_)) && added
}

pub(crate) fn up_toast(asked: Asked, port: u16, local: u16, host: &str) -> Option<(String, bool)> {
    let moved = local != port;
    match asked {
        Asked::Click => Some((tunnelled(port, local, host, moved), true)),
        Asked::Program => Some((program_tunnelled(port, local, host, moved), true)),
        Asked::Open if moved => Some((tunnelled(port, local, host, true), false)),
        Asked::Open | Asked::Back | Asked::Retry => None,
    }
}

pub(crate) fn came_back(asked: Asked, (port, local): (u16, u16), wanted: Option<u16>, host: &str) -> Option<String> {
    let wanted = wanted.filter(|wanted| *wanted != local)?;
    if !matches!(asked, Asked::Back | Asked::Retry) {
        return None;
    }
    Some(if local == port {
        format!("Port {port} on {host} is back on localhost:{local} (it was on localhost:{wanted})")
    } else {
        format!(
            "Port {port} on {host} is back on localhost:{local} (localhost:{wanted}, where it was, is taken on this {})",
            crate::desktop::here()
        )
    })
}

fn taken_on_this(port: u16, moved: bool) -> String {
    if moved { format!(" ({port} is taken on this {})", crate::desktop::here()) } else { String::new() }
}

fn tunnelled(port: u16, local: u16, host: &str, moved: bool) -> String {
    format!("Port {port} on {host} is tunnelled to localhost:{local}{}", taken_on_this(port, moved))
}

fn program_tunnelled(port: u16, local: u16, host: &str, moved: bool) -> String {
    format!("A program on {host} tunnelled port {port} to localhost:{local}{}", taken_on_this(port, moved))
}

#[cfg(test)]
pub(crate) fn test_hold(toward: Toward, port: u16, state: Held) -> Hold {
    let destination = match &toward {
        Toward::Server => "devbox".to_string(),
        Toward::Machine(_) => "vps.invalid".to_string(),
    };
    let label = destination.clone();
    let reached =
        Reached { toward, target: crate::test_util::vps_target(), boot: "boot".to_string(), label, destination };
    let mut hold = Hold::opening(&reached, port, 1, Asked::Click);
    hold.state = state;
    hold
}

#[cfg(test)]
mod tests;
