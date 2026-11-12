use super::{UpdateToast, Updating};
use crate::cli::{self, Answer, Said, Version};
use crate::notify::{AppState, InstallState};
use crate::store::ToastKind;

pub(crate) const UPDATE_JSON: &str = "usecornercase/cornercase-terminal#775";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Here {
    pub path: String,
    pub version: Option<Version>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Installed(String),
    Unchecked { version: String, host: String },
    Failed { why: String, printed: bool },
}

pub(crate) fn of(answer: anyhow::Result<Answer>, release: &str, host: Option<&str>, here: Option<&Here>) -> Outcome {
    let printed = answer.as_ref().is_ok_and(|a| a.code != Some(0) && !a.stdout.trim().is_empty());
    match (cli::said_of(answer), host) {
        (Said::Failed(why), _) => Outcome::Failed { why, printed },
        (Said::Done(_), Some(host)) => Outcome::Unchecked { version: release.to_string(), host: host.to_string() },
        (Said::Done(_), None) => checked(release, here),
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

impl Outcome {
    pub(crate) fn state(&self) -> InstallState {
        match self {
            Self::Installed(version) | Self::Unchecked { version, .. } => InstallState::Done(version.clone()),
            Self::Failed { why, .. } => InstallState::Failed(why.clone()),
        }
    }

    pub(crate) fn waits(&self) -> Option<String> {
        match self {
            Self::Failed { printed: true, .. } => Some(format!("Copy command needs {UPDATE_JSON}")),
            Self::Unchecked { host, .. } => {
                Some(format!("Not checked on {host} until its server restarts: needs {UPDATE_JSON}"))
            }
            _ => None,
        }
    }

    pub(crate) fn toast(&self, release: &str) -> Option<UpdateToast> {
        match self {
            Self::Failed { why, .. } => {
                Some(UpdateToast::install_failed("cornercase", release, why, Updating::Cornercase))
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

pub(crate) enum Where<'a> {
    Here(Option<&'a Here>),
    There(&'a str),
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
