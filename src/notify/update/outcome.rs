use super::{UpdateToast, Updating};
use crate::cli::{self, Answer, ByHand, Said, UPDATE_JSON_SINCE, UpdateAnswer, UpdateGate, Version};
use crate::desktop;
use crate::notify::{AppState, InstallState};
use crate::platform::Os;
use crate::store::ToastKind;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Here {
    pub path: String,
    pub version: Option<Version>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Installed(String),
    Unchecked { version: String, host: String },
    Manual(ByHand),
    Failed { why: String, printed: bool },
}

pub(crate) enum Where<'a> {
    Here(Option<&'a Here>),
    There(&'a str),
}

pub(crate) fn of(answer: &anyhow::Result<Answer>, release: &str, place: &Where<'_>) -> Outcome {
    let printed = answer.as_ref().is_ok_and(|a| a.code != Some(0) && !a.stdout.trim().is_empty());
    let failed = |why: String| Outcome::Failed { why, printed: false };
    match (cli::update_answer(answer), place) {
        (UpdateAnswer::Updated { version, .. }, _) => Outcome::Installed(version),
        (UpdateAnswer::UpToDate { version }, _) if reaches(&version, release) => Outcome::Installed(version),
        (UpdateAnswer::UpToDate { version }, place) => failed(none_newer(&version, place)),
        (UpdateAnswer::Manual(by_hand), _) => Outcome::Manual(by_hand),
        (UpdateAnswer::Refused, _) => failed(cli::refused_json()),
        (UpdateAnswer::Printed(Said::Failed(why)), _) => Outcome::Failed { why, printed },
        (UpdateAnswer::Printed(Said::Done(_)), Where::There(host)) => {
            Outcome::Unchecked { version: release.to_string(), host: (*host).to_string() }
        }
        (UpdateAnswer::Printed(Said::Done(_)), Where::Here(here)) => checked(release, *here),
    }
}

fn reaches(version: &str, release: &str) -> bool {
    match (Version::parse(version), Version::parse(release)) {
        (Some(version), Some(release)) => version >= release,
        _ => version == release,
    }
}

fn none_newer(version: &str, place: &Where<'_>) -> String {
    match place {
        Where::Here(_) => format!("cornercase update found no release newer than {version}"),
        Where::There(host) => format!("cornercase update on {host} found no release newer than {version}"),
    }
}

fn checked(release: &str, here: Option<&Here>) -> Outcome {
    let Some(here) = here else { return Outcome::Installed(release.to_string()) };
    let failed = |why: String| Outcome::Failed { why, printed: false };
    match here.version {
        Some(disk) if Version::parse(release).is_some_and(|out| disk < out) => {
            failed(format!("cornercase update finished, but {} is still {disk}", here.path))
        }
        Some(disk) => Outcome::Installed(disk.to_string()),
        None => failed(format!("cornercase update finished, but {} does not say its version", here.path)),
    }
}

fn needs(gate: UpdateGate, os: Os) -> Option<String> {
    if !gate.lacks() {
        return None;
    }
    let has = gate.here().map(|v| format!(" (it has {v})")).unwrap_or_default();
    Some(format!("cornercase {UPDATE_JSON_SINCE} or newer on this {}{has}", desktop::machine(os)))
}

impl Outcome {
    pub(crate) fn state(&self) -> InstallState {
        match self {
            Self::Installed(version) | Self::Unchecked { version, .. } => InstallState::Done(version.clone()),
            Self::Manual(by_hand) => InstallState::Manual(by_hand.clone()),
            Self::Failed { why, .. } => InstallState::Failed(why.clone()),
        }
    }

    pub(crate) fn waits(&self, gate: UpdateGate, os: Os) -> Option<String> {
        let needs = needs(gate, os);
        match self {
            Self::Failed { printed: true, .. } => needs.map(|needs| format!("Copy command needs {needs}")),
            Self::Unchecked { host, .. } => {
                let said = format!("Not checked on {host} until its server restarts");
                Some(needs.map_or(said.clone(), |needs| format!("{said}: needs {needs}")))
            }
            _ => None,
        }
    }

    pub(crate) fn toast(&self, release: &str, place: &Where<'_>) -> Option<UpdateToast> {
        match self {
            Self::Failed { why, .. } => {
                Some(UpdateToast::install_failed("cornercase", release, why, Updating::Cornercase))
            }
            Self::Manual(by_hand) => {
                let text = match place {
                    Where::Here(_) => format!("To install cornercase {}, run the command in Details", by_hand.latest),
                    Where::There(host) => format!(
                        "To install cornercase {} on {host}, run the command in Details on {host}",
                        by_hand.latest
                    ),
                };
                Some(UpdateToast { text, kind: ToastKind::NeedsYou, details: Some(Updating::Cornercase) })
            }
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Expected {
    pub version: String,
    pub boot: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CameBack {
    Now(String),
    Behind { running: String, expected: String },
}

pub(crate) fn came_back(expected: &Expected, app: &AppState) -> Option<CameBack> {
    let (boot, running) = (app.boot.as_ref()?, app.version.as_ref()?);
    if *boot == expected.boot {
        return None;
    }
    let took = match (Version::parse(running), Version::parse(&expected.version)) {
        (Some(running), Some(expected)) => running >= expected,
        _ => *running == expected.version,
    };
    Some(if took {
        CameBack::Now(running.clone())
    } else {
        CameBack::Behind { running: running.clone(), expected: expected.version.clone() }
    })
}

impl CameBack {
    pub(crate) fn toast(&self, place: &Where<'_>) -> UpdateToast {
        let (text, kind) = match (self, place) {
            (Self::Now(version), Where::There(host)) => {
                (format!("cornercase on {host} is now {version}"), ToastKind::Done)
            }
            (Self::Now(version), Where::Here(_)) => (format!("cornercase is now {version}"), ToastKind::Done),
            (Self::Behind { running, expected }, Where::There(host)) => {
                (format!("The server on {host} came back on cornercase {running}, not {expected}"), ToastKind::Error)
            }
            (Self::Behind { running, expected }, Where::Here(here)) => {
                let text = format!("The server came back on cornercase {running}, not {expected}");
                (format!("{text}{}", binary_hint(*here)), ToastKind::Error)
            }
        };
        UpdateToast { text, kind, details: None }
    }
}

fn binary_hint(here: Option<&Here>) -> String {
    match here {
        Some(Here { path, version: Some(version) }) => format!("; this window runs {path}, which is {version}"),
        Some(Here { path, version: None }) => format!("; this window runs {path}"),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests;
