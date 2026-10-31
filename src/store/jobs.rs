use std::collections::HashSet;
use std::time::{Duration, Instant};

pub(crate) const SHOW_AFTER: Duration = Duration::from_millis(300);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Busy {
    key: String,
    label: String,
}

impl Busy {
    pub(crate) fn new(key: impl Into<String>, label: impl Into<String>) -> Self {
        Self { key: key.into(), label: label.into() }
    }

    pub(crate) fn key(&self) -> &str {
        &self.key
    }

    pub(crate) fn remove_worktree(workspace: u64, name: &str) -> Self {
        Self::new(removal_key(workspace), format!("Removing {name}…"))
    }
}

fn removal_key(workspace: u64) -> String {
    format!("remove-worktree:{workspace}")
}

#[derive(Default)]
pub(crate) struct Jobs {
    running: Vec<(u64, Instant, Busy)>,
    next: u64,
    leaving: HashSet<u64>,
}

impl Jobs {
    pub(crate) fn start(&mut self, busy: Busy) -> u64 {
        self.next += 1;
        self.running.push((self.next, Instant::now(), busy));
        self.next
    }

    pub(crate) fn end(&mut self, id: u64) {
        self.running.retain(|(job, _, _)| *job != id);
    }

    pub(crate) fn busy(&self, key: &str) -> bool {
        self.running.iter().any(|(_, _, busy)| busy.key == key)
    }

    pub(crate) fn doing(&self, key: &str) -> Option<&str> {
        self.running.iter().rev().find(|(_, _, busy)| busy.key == key).map(|(_, _, busy)| busy.label.as_str())
    }

    pub(crate) fn summary(&self, now: Instant) -> Option<String> {
        let shown: Vec<&Busy> =
            self.running.iter().filter(|(_, at, _)| now.duration_since(*at) >= SHOW_AFTER).map(|(_, _, b)| b).collect();
        let latest = shown.last()?;
        Some(match shown.len() {
            1 => latest.label.clone(),
            n => format!("{} and {} more", latest.label, n - 1),
        })
    }

    pub(crate) fn removed(&mut self, workspace: u64) {
        self.leaving.insert(workspace);
    }

    pub(crate) fn removing(&self, workspace: u64) -> bool {
        self.leaving.contains(&workspace) || self.busy(&removal_key(workspace))
    }

    pub(crate) fn keep_leaving(&mut self, listed: impl Fn(u64) -> bool) {
        self.leaving.retain(|workspace| listed(*workspace));
    }
}

#[cfg(test)]
mod tests;
