use std::path::PathBuf;
use std::process::Stdio;

use crate::platform;

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
    pub(crate) fn start(&self) {
        let _ = platform::command(&self.program)
            .args(&self.args)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn();
    }
}

#[cfg(test)]
mod tests;
