use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Result;
use parking_lot::Mutex;

use super::{Target, read_output, spawn};

static KEPT: Kept = Kept::empty();

struct Kept {
    children: Mutex<Vec<(u64, Child)>>,
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
    ) -> Result<()> {
        let mut children = self.children.lock();
        let mut child = spawn(&mut command)?;
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        read_output(&mut child, on_line, move |why| {
            self.reap(id);
            on_end(why);
        })?;
        children.push((id, child));
        Ok(())
    }

    fn reap(&self, id: u64) {
        let ended = {
            let mut children = self.children.lock();
            children.iter().position(|(kept, _)| *kept == id).map(|at| children.swap_remove(at))
        };
        if let Some((_, mut child)) = ended {
            let _ = child.wait();
        }
    }

    fn stop(&self) {
        for (_, child) in self.children.lock().iter_mut() {
            ask_to_end(child);
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
