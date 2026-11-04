use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{ChildStdout, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use super::{Probe, closed, shell_args, shell_path};
use crate::desktop::Lookup;
use crate::platform;

const VERSION_WAIT: Duration = Duration::from_secs(5);
const SHELL_WAIT: Duration = Duration::from_secs(5);

pub(crate) struct System {
    lookup: Lookup,
}

impl System {
    pub(crate) fn here() -> Self {
        Self { lookup: Lookup::here() }
    }
}

impl Probe for System {
    fn find(&self, dirs: &[PathBuf], name: &str) -> Option<PathBuf> {
        self.lookup.find_in(dirs, name)
    }

    fn is_program(&self, path: &Path) -> bool {
        self.lookup.is_program(path)
    }

    fn version(&self, path: &Path) -> Result<Vec<String>, String> {
        let mut cmd = platform::command(path);
        cmd.arg("--version");
        lines_within(&mut cmd, VERSION_WAIT, |lines| !lines.is_empty())
    }

    fn login_path(&self, shell: &Path) -> Option<String> {
        let mut cmd = platform::command(shell);
        cmd.args(shell_args(shell));
        shell_path(&lines_within(&mut cmd, SHELL_WAIT, closed).ok()?)
    }
}

fn lines_within(cmd: &mut Command, limit: Duration, done: fn(&[String]) -> bool) -> Result<Vec<String>, String> {
    let deadline = Instant::now() + limit;
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let (tx, rx) = mpsc::channel();
    if let Some(stdout) = child.stdout.take() {
        thread::spawn(move || forward(stdout, &tx));
    }
    let lines = gather(&rx, deadline, done);
    let _ = child.kill();
    let _ = child.wait();
    Ok(lines)
}

fn forward(stdout: ChildStdout, tx: &mpsc::Sender<String>) {
    let lines = BufReader::new(stdout).lines().map_while(Result::ok);
    for line in lines {
        if tx.send(line).is_err() {
            return;
        }
    }
}

fn gather(rx: &mpsc::Receiver<String>, deadline: Instant, done: fn(&[String]) -> bool) -> Vec<String> {
    let mut lines = Vec::new();
    while !done(&lines) {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(line) => lines.push(line),
            Err(_) => break,
        }
    }
    lines
}

#[cfg(test)]
mod tests;
