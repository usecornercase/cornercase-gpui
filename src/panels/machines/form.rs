use crate::cli;
use crate::model::machines::{Machine, MachineIcon};

pub(super) const DEFAULT_FOLDER: &str = "~/code";
const NAME_MOST: usize = 20;
const DESTINATION_MOST: usize = 255;
const HOSTS_SHOWN: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Draft {
    pub name: String,
    pub icon: MachineIcon,
    pub destination: String,
    pub folder: String,
}

impl Draft {
    pub(super) fn new() -> Self {
        Self {
            name: String::new(),
            icon: MachineIcon::default(),
            destination: String::new(),
            folder: DEFAULT_FOLDER.to_string(),
        }
    }

    pub(super) fn of(machine: &Machine) -> Self {
        Self {
            name: machine.name.clone(),
            icon: machine.icon,
            destination: machine.destination.clone(),
            folder: machine.folder.clone(),
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Refusals {
    pub name: Option<String>,
    pub destination: Option<String>,
    pub folder: Option<String>,
}

impl Refusals {
    pub(super) fn any(&self) -> bool {
        self.name.is_some() || self.destination.is_some() || self.folder.is_some()
    }
}

pub(super) fn others<'a>(machines: &'a [Machine], was: Option<&str>) -> Vec<&'a Machine> {
    let edited = was.and_then(|was| machines.iter().position(|m| m.name.eq_ignore_ascii_case(was)));
    machines.iter().enumerate().filter(|(at, _)| Some(*at) != edited).map(|(_, machine)| machine).collect()
}

fn name_refusal(name: &str, others: &[&Machine]) -> Option<String> {
    let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-');
    if name.chars().count() > NAME_MOST || !name.chars().all(allowed) {
        return Some("Use letters, digits, `.`, `_` and `-`, at most 20 characters".to_string());
    }
    if name.eq_ignore_ascii_case("all") {
        return Some("`all` is kept for every machine at once".to_string());
    }
    let taken = others.iter().find(|other| other.name.eq_ignore_ascii_case(name))?;
    Some(format!("Another machine is already called `{}`", taken.name))
}

fn destination_refusal(destination: &str, others: &[&Machine]) -> Option<String> {
    if destination.starts_with('-') {
        return Some("A destination cannot start with `-`".to_string());
    }
    if destination.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Some("A destination cannot hold spaces or control characters".to_string());
    }
    if destination.len() > DESTINATION_MOST {
        return Some("A destination is at most 255 bytes".to_string());
    }
    let taken = others.iter().find(|other| other.destination == destination)?;
    Some(format!("The machine `{}` already uses this destination", taken.name))
}

fn folder_refusal(folder: &str) -> Option<String> {
    if folder.chars().any(char::is_control) {
        return Some("A folder cannot hold control characters".to_string());
    }
    let shaped = folder.starts_with('/') || folder == "~" || folder.starts_with("~/");
    (!shaped).then(|| "Use an absolute path or one starting with ~/".to_string())
}

fn checked(text: &str, required: bool, what: &str, refusal: impl FnOnce() -> Option<String>) -> Option<String> {
    if text.is_empty() {
        return required.then(|| format!("{what} is required"));
    }
    refusal()
}

pub(super) fn refusals(draft: &Draft, others: &[&Machine], required: bool) -> Refusals {
    Refusals {
        name: checked(&draft.name, required, "The name", || name_refusal(&draft.name, others)),
        destination: checked(&draft.destination, required, "The destination", || {
            destination_refusal(&draft.destination, others)
        }),
        folder: checked(&draft.folder, required, "The folder", || folder_refusal(&draft.folder)),
    }
}

pub(super) fn hosts_offered(hosts: &[String], typed: &str, others: &[&Machine]) -> Vec<String> {
    let typed = typed.to_lowercase();
    let free = |host: &&String| !others.iter().any(|other| other.destination == **host);
    let matches = |host: &&String| host.to_lowercase().contains(&typed) && host.to_lowercase() != typed;
    hosts.iter().filter(free).filter(matches).take(HOSTS_SHOWN).cloned().collect()
}

pub(super) fn destination_rule(server: Option<&str>) -> String {
    match server {
        None => "What you give ssh: user@host, or a Host of your ~/.ssh/config".to_string(),
        Some(server) => format!("What ssh takes on {server}: user@host, or a Host of its ~/.ssh/config"),
    }
}

fn version_needed(here: Option<&str>) -> String {
    here.filter(|version| !version.is_empty()).map_or_else(|| cli::MINIMUM.to_string(), str::to_string)
}

pub(super) fn empty_needs(here: Option<&str>) -> String {
    let version = version_needed(here);
    format!("It needs Linux or macOS, cornercase {version} or newer, and an ssh key that never asks for anything.")
}

pub(super) fn setup_note(here: Option<&str>, destination: &str, server: Option<&str>) -> String {
    let version = version_needed(here);
    let target = if destination.is_empty() { "you@host" } else { destination };
    let from = server.map(|server| format!(" from {server}")).unwrap_or_default();
    format!(
        "It must run Linux or macOS. Install cornercase {version} or newer there, and connect once by hand{from} with \
         `ssh {target} true` so the key and the host are known, since the link never asks."
    )
}

#[cfg(test)]
mod tests;
