use anyhow::{Result, bail};

use super::Target;

const PARTIAL: i32 = 3;

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

#[cfg(test)]
mod tests;
