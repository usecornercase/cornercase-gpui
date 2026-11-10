use std::path::Path;

use anyhow::Result;
use serde::Deserialize;

use super::{Binary, Target, Version};

const OLDER: i32 = 5;
const NAME: &str = "cornercase";
const SILENT: &str = "cornercase did not say why";

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Answer {
    pub(crate) code: Option<i32>,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Said {
    Done(String),
    Failed(String),
}

fn refusal(code: Option<i32>, stdout: &str) -> Option<Older> {
    if code != Some(OLDER) {
        return None;
    }
    let fact = stdout.lines().find_map(|line| serde_json::from_str::<Refusal>(line).ok())?.error;
    (fact.code == "older").then_some(fact.older)
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

pub(crate) fn start_server(args: &[&str], theme: &str) -> Start {
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

pub(crate) fn answer_on<S: AsRef<str>>(target: &Target, args: &[S]) -> Result<Answer> {
    Ok(answer(&super::output(&mut target.command(args))?))
}

pub(crate) fn answer_at<S: AsRef<str>>(path: &Path, args: &[S]) -> Result<Answer> {
    let mut cmd = Target::Local.command_with(&Binary::plain(path.into()), args);
    Ok(answer(&super::output(&mut cmd)?))
}

fn answer(out: &std::process::Output) -> Answer {
    Answer {
        code: out.status.code(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

fn said(answer: &Answer) -> Said {
    let out = answer.stdout.trim();
    if answer.code == Some(0) {
        return Said::Done(out.to_string());
    }
    if !out.is_empty() {
        return Said::Failed(out.to_string());
    }
    let why = super::reason_of(&answer.stderr);
    Said::Failed(if why.is_empty() { SILENT.to_string() } else { why })
}

pub(crate) fn said_of(result: Result<Answer>) -> Said {
    match result {
        Ok(answer) => said(&answer),
        Err(e) => Said::Failed(super::reason(&e)),
    }
}

#[cfg(test)]
mod tests;
