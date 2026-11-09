use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Stdio;

use super::bundle;
use crate::platform::{self, Os};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Relaunch {
    pub program: PathBuf,
    pub args: Vec<String>,
}

pub(crate) fn relaunch_args(flags: &[String], tab: Option<u64>) -> Vec<String> {
    let shown = tab.into_iter().flat_map(|tab| ["--tab".to_string(), tab.to_string()]);
    shown.chain(flags.iter().cloned()).collect()
}

impl Relaunch {
    pub(crate) fn command_line(&self, os: Os, pid: u32) -> (PathBuf, Vec<OsString>) {
        match bundle::of(&self.program).filter(|_| os == Os::Mac) {
            Some(app) => bundle::reopen(&app, pid, &self.args),
            None => (self.program.clone(), self.args.iter().map(OsString::from).collect()),
        }
    }

    pub(crate) fn start(&self) -> std::io::Result<()> {
        let (program, args) = self.command_line(Os::CURRENT, std::process::id());
        platform::command(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map(drop)
    }
}

#[cfg(test)]
mod tests;
