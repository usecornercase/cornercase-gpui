use std::path::Path;

use anyhow::{Result, bail};

use super::Target;

const PARTIAL: i32 = 3;
const SILENT: &str = "cornercase did not say why";

pub(crate) fn run_partial<S: AsRef<str>>(args: &[S]) -> Result<String> {
    run_partial_on(&super::target(), args)
}

pub(crate) fn run_partial_on<S: AsRef<str>>(target: &Target, args: &[S]) -> Result<String> {
    let out = super::output(&mut target.command(args))?;
    let name = args.first().map(AsRef::as_ref).unwrap_or_default();
    answer(name, out.status.code(), &out.stdout, &out.stderr)
}

pub(crate) fn run_printed<S: AsRef<str>>(args: &[S]) -> Result<String> {
    let out = super::output(&mut super::command(args))?;
    let name = args.first().map(AsRef::as_ref).unwrap_or_default();
    printed(name, out.status.code(), &out.stdout, &out.stderr)
}

fn answer(name: &str, code: Option<i32>, stdout: &[u8], stderr: &[u8]) -> Result<String> {
    if matches!(code, Some(0 | PARTIAL)) {
        return Ok(String::from_utf8_lossy(stdout).into_owned());
    }
    refused(name, stderr)
}

fn printed(name: &str, code: Option<i32>, stdout: &[u8], stderr: &[u8]) -> Result<String> {
    if code == Some(0) || !String::from_utf8_lossy(stdout).trim().is_empty() {
        return Ok(String::from_utf8_lossy(stdout).into_owned());
    }
    refused(name, stderr)
}

fn refused(name: &str, stderr: &[u8]) -> Result<String> {
    let err = String::from_utf8_lossy(stderr);
    let err = err.trim().trim_start_matches("Error: ");
    bail!("cornercase {name}: {}", if err.is_empty() { "failed" } else { err })
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

pub(crate) fn answer_on<S: AsRef<str>>(target: &Target, args: &[S]) -> Result<Answer> {
    Ok(reply_of(&super::output(&mut target.command(args))?))
}

pub(crate) fn answer_at<S: AsRef<str>>(path: &Path, args: &[S]) -> Result<Answer> {
    let mut cmd = Target::Local.command_with(&super::Binary::plain(path.into()), args);
    Ok(reply_of(&super::output(&mut cmd)?))
}

fn reply_of(out: &std::process::Output) -> Answer {
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
    let why = super::reason_of(&answer.stderr);
    let parts: Vec<&str> = [out, why.as_str()].into_iter().filter(|part| !part.is_empty()).collect();
    Said::Failed(if parts.is_empty() { SILENT.to_string() } else { parts.join("\n") })
}

pub(crate) fn said_of(result: Result<Answer>) -> Said {
    match result {
        Ok(answer) => said(&answer),
        Err(e) => Said::Failed(super::reason(&e)),
    }
}

#[cfg(test)]
mod tests;
