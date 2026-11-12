use std::process::{Command, Stdio};

use anyhow::Result;

use super::Target;
use super::kept::{End, Kept};

static KEPT: Kept = Kept::empty(End::LetGo);

fn command() -> Command {
    let mut command = Target::Local.command(&["plugin", "windows", "--json"]);
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    command
}

pub(crate) fn keep_window_processes(
    on_line: impl FnMut(String) + Send + 'static,
    on_end: impl FnOnce(String) + Send + 'static,
) -> Result<Option<String>> {
    KEPT.start(command(), on_line, move |why, _| on_end(why)).map(|(_, untied)| untied)
}

pub(crate) fn stop_window_processes() {
    KEPT.stop();
}

#[cfg(test)]
mod tests;
