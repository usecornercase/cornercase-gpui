use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Result;
use parking_lot::Mutex;

use super::{Target, read_output, spawn};
use crate::platform::Os;

#[cfg(windows)]
mod win32;
#[cfg(not(windows))]
#[path = "window_processes/no_win32.rs"]
mod win32;

static KEPT: Kept = Kept::empty();

struct Running {
    id: u64,
    child: Child,
    job: Option<win32::Job>,
}

impl Running {
    fn let_go(&self) {
        if let Some(job) = &self.job {
            win32::let_go(job);
        }
    }
}

struct Kept {
    children: Mutex<Vec<Running>>,
    next: AtomicU64,
}

impl Kept {
    const fn empty() -> Self {
        Self { children: Mutex::new(Vec::new()), next: AtomicU64::new(1) }
    }

    fn start(
        &'static self,
        mut command: Command,
        on_line: impl FnMut(String) + Send + 'static,
        on_end: impl FnOnce(String) + Send + 'static,
    ) -> Result<Option<String>> {
        let mut children = self.children.lock();
        let mut child = spawn(&mut command)?;
        let (job, untied) = tie(Os::CURRENT, &child);
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        read_output(&mut child, on_line, move |why| {
            self.reap(id);
            on_end(why);
        })?;
        children.push(Running { id, child, job });
        Ok(untied)
    }

    fn reap(&self, id: u64) {
        let ended = {
            let mut children = self.children.lock();
            children.iter().position(|running| running.id == id).map(|at| children.swap_remove(at))
        };
        if let Some(mut running) = ended {
            let _ = running.child.wait();
            running.let_go();
        }
    }

    fn stop(&self) {
        for running in self.children.lock().iter_mut() {
            ask_to_end(&mut running.child);
            running.let_go();
        }
    }
}

fn tie(os: Os, child: &Child) -> (Option<win32::Job>, Option<String>) {
    match os {
        Os::Windows => match win32::tie(child) {
            Ok(job) => (Some(job), None),
            Err(why) => (None, Some(why)),
        },
        Os::Mac | Os::Linux => (None, None),
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
) -> Result<Option<String>> {
    KEPT.start(command(), on_line, on_end)
}

pub(crate) fn stop_window_processes() {
    KEPT.stop();
}

#[cfg(test)]
mod tests;
