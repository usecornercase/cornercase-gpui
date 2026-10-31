use std::process::Stdio;

use anyhow::{Context as _, Result, bail};

const PARTIAL: i32 = 3;

pub(crate) fn run_partial<S: AsRef<str>>(args: &[S]) -> Result<String> {
    let out = super::command(args).stdin(Stdio::null()).output().context("could not run cornercase")?;
    let name = args.first().map(AsRef::as_ref).unwrap_or_default();
    answer(name, out.status.code(), &out.stdout, &out.stderr)
}

fn answer(name: &str, code: Option<i32>, stdout: &[u8], stderr: &[u8]) -> Result<String> {
    if matches!(code, Some(0 | PARTIAL)) {
        return Ok(String::from_utf8_lossy(stdout).into_owned());
    }
    let err = String::from_utf8_lossy(stderr);
    let err = err.trim().trim_start_matches("Error: ");
    bail!("cornercase {name}: {}", if err.is_empty() { "failed" } else { err })
}

#[cfg(test)]
mod tests;
