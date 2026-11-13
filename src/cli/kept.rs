use std::process::{Child, Command};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Result;
use parking_lot::Mutex;

use super::{read_output, spawn};
use crate::platform::Os;

#[cfg(windows)]
mod win32;
#[cfg(not(windows))]
#[path = "kept/no_win32.rs"]
mod win32;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum End {
    LetGo,
    TakeAlong,
}

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

    fn end(&mut self, how: End) {
        ask_to_end(&mut self.child);
        match how {
            End::LetGo => self.let_go(),
            End::TakeAlong => self.job = None,
        }
    }
}

pub(super) struct Kept {
    children: Mutex<Vec<Running>>,
    next: AtomicU64,
    how: End,
}

impl Kept {
    pub(super) const fn empty(how: End) -> Self {
        Self { children: Mutex::new(Vec::new()), next: AtomicU64::new(1), how }
    }

    pub(super) fn start(
        &'static self,
        mut command: Command,
        on_line: impl FnMut(String) + Send + 'static,
        on_end: impl FnOnce(String, Option<i32>) + Send + 'static,
    ) -> Result<(u64, Option<String>)> {
        let mut children = self.children.lock();
        let mut child = spawn(&mut command)?;
        let (job, untied) = tie(Os::CURRENT, &child);
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        read_output(&mut child, on_line, move |why| {
            let code = self.reap(id);
            on_end(why, code);
        })?;
        children.push(Running { id, child, job });
        Ok((id, untied))
    }

    fn reap(&self, id: u64) -> Option<i32> {
        let ended = {
            let mut children = self.children.lock();
            children.iter().position(|running| running.id == id).map(|at| children.swap_remove(at))
        };
        let mut running = ended?;
        let code = running.child.wait().ok().and_then(|status| status.code());
        if self.how == End::LetGo {
            running.let_go();
        }
        code
    }

    pub(super) fn stop(&self) {
        for running in self.children.lock().iter_mut() {
            running.end(self.how);
        }
    }

    pub(super) fn stop_one(&self, id: u64) {
        if let Some(running) = self.children.lock().iter_mut().find(|running| running.id == id) {
            running.end(self.how);
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

#[cfg(test)]
mod tests;
