use super::form::Draft;
use crate::keymap::{self, Command};
use crate::model::machines::Machine;
use crate::platform::Os;
use crate::store::{Busy, args};

const THIS_COMPUTER: &str = "this computer";
const HERE_LOCAL: &str = "Runs the server you are using; it links to each machine below";
const HERE_REMOTE: &str = "Runs the server this window is connected to; it links to each machine below";

pub(super) fn here_words(server: Option<&str>) -> (&str, &'static str) {
    match server {
        None => (THIS_COMPUTER, HERE_LOCAL),
        Some(server) => (server, HERE_REMOTE),
    }
}

pub(super) fn window_note(os: Os) -> String {
    let keys = keymap::keys_of(Command::Connect, os).unwrap_or_default();
    format!("To move this whole window to another server instead, press {keys}.")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Doing {
    Retry,
    Update,
    Remove,
}

impl Doing {
    pub(super) fn words(self) -> &'static str {
        match self {
            Self::Retry => "Retrying…",
            Self::Update => "updating…",
            Self::Remove => "removing…",
        }
    }

    pub(super) fn job(self, name: &str) -> Busy {
        let label = match self {
            Self::Retry => format!("Trying {name} again…"),
            Self::Update => format!("Updating cornercase on {name}…"),
            Self::Remove => format!("Removing {name}…"),
        };
        Busy::new(format!("machine:{name}"), label)
    }

    pub(super) fn command(self, name: &str) -> Vec<String> {
        let action = match self {
            Self::Retry => "retry",
            Self::Update => "update",
            Self::Remove => "remove",
        };
        args(&["machines", action, "--json", "--", name])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Ask {
    pub title: String,
    pub message: &'static str,
    pub button: &'static str,
    pub name: String,
    pub doing: Doing,
}

pub(super) fn remove_ask(name: &str) -> Ask {
    Ask {
        title: format!("Remove the machine {name}?"),
        message: "Its link ends here. Its server keeps running there and nothing there is touched: adding it again \
                  brings it back.",
        button: "Remove",
        name: name.to_string(),
        doing: Doing::Remove,
    }
}

pub(super) fn update_ask(name: &str) -> Ask {
    Ask {
        title: format!("Update cornercase on {name}?"),
        message: "This runs `cornercase update` there, which installs this version and restarts nothing: its server \
                  keeps running the one it has until it restarts there.",
        button: "Update",
        name: name.to_string(),
        doing: Doing::Update,
    }
}

pub(super) struct Saving {
    pub command: Vec<String>,
    pub job: Busy,
}

fn flag(name: &str, value: &str) -> String {
    format!("--{name}={value}")
}

pub(super) fn add(draft: &Draft) -> Saving {
    let mut command = args(&["machines", "add", "--json"]);
    command.push(flag("icon", draft.icon.name()));
    command.push(flag("folder", &draft.folder));
    command.extend(args(&["--", &draft.name, &draft.destination]));
    Saving { command, job: Busy::new("machine-form", format!("Adding {}…", draft.name)) }
}

pub(super) fn edit(was: &Draft, draft: &Draft) -> Option<Saving> {
    let changed = [
        (was.name != draft.name).then(|| flag("name", &draft.name)),
        (was.icon != draft.icon).then(|| flag("icon", draft.icon.name())),
        (was.destination != draft.destination).then(|| flag("destination", &draft.destination)),
        (was.folder != draft.folder).then(|| flag("folder", &draft.folder)),
    ];
    let changed: Vec<String> = changed.into_iter().flatten().collect();
    if changed.is_empty() {
        return None;
    }
    let mut command = args(&["machines", "edit", "--json"]);
    command.extend(changed);
    command.extend(args(&["--", &was.name]));
    Some(Saving { command, job: Busy::new("machine-form", format!("Saving {}…", was.name)) })
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Listed<'a> {
    pub at: usize,
    pub machine: &'a Machine,
    pub shadowed: bool,
}

pub(super) fn listed(machines: &[Machine]) -> Vec<Listed<'_>> {
    let earlier = |at: usize, name: &str| machines[..at].iter().any(|m| m.name.eq_ignore_ascii_case(name));
    machines
        .iter()
        .enumerate()
        .map(|(at, machine)| Listed { at, machine, shadowed: earlier(at, &machine.name) })
        .collect()
}

pub(super) fn shows_doing(doing: Doing, row: Listed<'_>) -> bool {
    !row.shadowed && (doing != Doing::Retry || row.machine.can_retry())
}

pub(super) fn edits(row: Listed<'_>, name: &str) -> bool {
    !row.shadowed && row.machine.name == name
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Note {
    Error(String),
    Hint(String),
}

const SHADOWED: &str = "commands reach the first entry of a name, so this one can only be fixed in the file";

pub(super) fn notes(row: Listed<'_>) -> Vec<Note> {
    let machine = row.machine;
    let problem = machine.problem.as_deref().map(|problem| {
        let end = if row.shadowed { format!("; {SHADOWED}") } else { String::new() };
        Note::Error(format!("In config.json: {problem}{end}"))
    });
    let error = machine.error.clone().map(Note::Error);
    let behind = machine.server_behind().map(Note::Hint);
    [problem, error, behind].into_iter().flatten().collect()
}

#[cfg(test)]
mod tests;
