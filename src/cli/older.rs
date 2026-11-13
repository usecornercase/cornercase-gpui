use serde::Deserialize;

use super::Version;

const OLDER: i32 = 5;
const NAME: &str = "cornercase";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct Older {
    pub(crate) destination: String,
    #[serde(default)]
    pub(crate) there: Option<String>,
    pub(crate) here: String,
    #[serde(default)]
    pub(crate) update: Vec<String>,
}

#[derive(Deserialize)]
struct Refusal {
    error: Fact,
}

#[derive(Deserialize)]
struct Fact {
    code: String,
    #[serde(flatten)]
    older: Older,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Started {
    pub(crate) version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Behind {
    pub(crate) destination: String,
    pub(crate) server: String,
    pub(crate) here: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Start {
    Up(Started),
    Older(Older),
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Step {
    Open,
    Behind(Behind),
    Older(Older),
    Lost(String),
}

fn refusal(code: Option<i32>, stdout: &str) -> Option<Older> {
    if code != Some(OLDER) {
        return None;
    }
    let fact = stdout.lines().find_map(|line| serde_json::from_str::<Refusal>(line).ok())?.error;
    (fact.code == "older").then_some(fact.older)
}

#[derive(Debug, Clone)]
struct OlderAnswer {
    older: Older,
    said: String,
}

impl std::fmt::Display for OlderAnswer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.said)
    }
}

impl std::error::Error for OlderAnswer {}

pub(crate) fn refused_older(name: &str, code: Option<i32>, stdout: &[u8], stderr: &[u8]) -> Option<anyhow::Error> {
    let older = refusal(code, &String::from_utf8_lossy(stdout))?;
    let said = super::failure(name, code, stderr).to_string();
    Some(OlderAnswer { older, said }.into())
}

pub(crate) fn older_refusal(error: &anyhow::Error) -> Option<&Older> {
    error.downcast_ref::<OlderAnswer>().map(|answer| &answer.older)
}

fn start(code: Option<i32>, stdout: &[u8], stderr: &[u8]) -> Start {
    if code == Some(0) {
        return Start::Up(serde_json::from_slice(stdout).unwrap_or_default());
    }
    match refusal(code, &String::from_utf8_lossy(stdout)) {
        Some(older) => Start::Older(older),
        None => Start::Failed(super::failure("start-server", code, stderr).to_string()),
    }
}

pub(crate) fn start_server(args: &[String], theme: &str) -> Start {
    match super::output_on(&super::target(), args, Some(theme.as_bytes())) {
        Ok(out) => start(out.status.code(), &out.stdout, &out.stderr),
        Err(e) => Start::Failed(e.to_string()),
    }
}

pub(crate) fn step(start: Start, updated: Option<&Older>) -> Step {
    match start {
        Start::Up(started) => match updated.and_then(|older| behind(&started, older)) {
            Some(behind) => Step::Behind(behind),
            None => Step::Open,
        },
        Start::Older(older) => Step::Older(older),
        Start::Failed(why) => Step::Lost(why),
    }
}

fn behind(started: &Started, older: &Older) -> Option<Behind> {
    let server = Version::parse(&started.version)?;
    let here = Version::parse(&older.here)?;
    (server < here).then(|| Behind {
        destination: older.destination.clone(),
        server: started.version.clone(),
        here: older.here.clone(),
    })
}

pub(crate) fn update_args(older: &Older) -> Option<Vec<String>> {
    let (first, rest) = older.update.split_first()?;
    (first == NAME && !rest.is_empty()).then(|| rest.to_vec())
}

#[cfg(test)]
mod tests;
