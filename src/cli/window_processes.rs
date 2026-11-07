use std::process::{Child, Command, Stdio};

use anyhow::Result;
use parking_lot::Mutex;

use super::{Target, read_output, spawn};

static KEPT: Kept = Kept::empty();

struct Kept(Mutex<Option<Child>>);

impl Kept {
    const fn empty() -> Self {
        Self(Mutex::new(None))
    }

    fn start(
        &'static self,
        mut command: Command,
        on_line: impl FnMut(String) + Send + 'static,
        on_end: impl FnOnce(String) + Send + 'static,
    ) -> Result<()> {
        let mut slot = self.0.lock();
        let mut child = spawn(&mut command)?;
        read_output(&mut child, on_line, move |why| {
            self.reap();
            on_end(why);
        })?;
        *slot = Some(child);
        Ok(())
    }

    fn reap(&self) {
        let ended = self.0.lock().take();
        if let Some(mut child) = ended {
            let _ = child.wait();
        }
    }

    fn stop(&self) {
        let kept = self.0.lock().take();
        if let Some(mut child) = kept {
            ask_to_end(&mut child);
        }
    }
}

#[cfg(unix)]
fn ask_to_end(child: &mut Child) {
    if let Ok(pid) = libc::pid_t::try_from(child.id()) {
        unsafe { libc::kill(pid, libc::SIGTERM) };
    }
}

#[cfg(not(unix))]
fn ask_to_end(child: &mut Child) {
    let _ = child.kill();
}

fn command() -> Command {
    let mut command = Target::Local.command(&["plugin", "windows", "--json"]);
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    command
}

pub(crate) fn keep_window_processes(
    on_line: impl FnMut(String) + Send + 'static,
    on_end: impl FnOnce(String) + Send + 'static,
) -> Result<()> {
    KEPT.start(command(), on_line, on_end)
}

pub(crate) fn stop_window_processes() {
    KEPT.stop();
}

#[cfg(test)]
mod tests;
