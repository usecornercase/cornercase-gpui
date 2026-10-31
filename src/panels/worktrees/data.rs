use std::fmt::Write as _;

use serde::Deserialize;
use serde_json::Value;

use crate::panels::files::disk::human_size;
use crate::panels::flows::model::{ago, epoch};

const IDLE_DAYS: i64 = 7;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct List {
    pub(super) worktrees: Vec<Worktree>,
}

#[expect(clippy::struct_excessive_bools, reason = "mirrors the flags of `worktrees list --json`")]
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Worktree {
    pub(super) path: String,
    pub(super) kind: String,
    pub(super) project_name: String,
    pub(super) branch: Option<String>,
    pub(super) workspace: Option<u64>,
    pub(super) last_changed: Option<String>,
    pub(super) changed: Option<u64>,
    pub(super) unpushed: Option<u64>,
    pub(super) merged: bool,
    pub(super) no_commits: bool,
    pub(super) upstream_gone: bool,
    pub(super) lock: Option<Value>,
    pub(super) running: Vec<String>,
    pub(super) outside: Vec<Value>,
    pub(super) removing: bool,
    pub(super) run_role: bool,
    pub(super) safe: bool,
    pub(super) risks: Vec<String>,
    pub(super) error: Option<String>,
    pub(super) size: Option<Size>,
    pub(super) size_error: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Size {
    pub(super) bytes: u64,
    pub(super) builds_bytes: u64,
    pub(super) builds: Vec<Build>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Build {
    pub(super) path: String,
    pub(super) bytes: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Freed {
    pub(super) bytes: u64,
    pub(super) folders: Vec<Build>,
    pub(super) failed: Vec<Value>,
    pub(super) running: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Tone {
    Plain,
    Warn,
    Good,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Order {
    #[default]
    SafeFirst,
    LargestFirst,
}

impl Worktree {
    pub(super) fn title(&self) -> String {
        format!("{} › {}", self.project_name, self.branch.as_deref().unwrap_or("(no branch)"))
    }

    pub(super) fn matches(&self, filter: &str) -> bool {
        let filter = filter.trim().to_lowercase();
        filter.is_empty()
            || self.project_name.to_lowercase().contains(&filter)
            || self.branch.as_deref().is_some_and(|b| b.to_lowercase().contains(&filter))
    }

    pub(super) fn age(&self, now: i64) -> String {
        self.last_changed.as_deref().map(|t| ago(t, now)).unwrap_or_default()
    }

    fn idle(&self, now: i64) -> bool {
        self.last_changed.as_deref().and_then(epoch).is_some_and(|t| now - t >= IDLE_DAYS * 86_400)
    }

    pub(super) fn marks(&self) -> Vec<(String, Tone)> {
        let mut marks = Vec::new();
        if self.kind != "linked" && !self.kind.is_empty() {
            marks.push((self.kind.replace('-', " "), Tone::Warn));
        }
        if let Some(changed) = self.changed.filter(|&n| n > 0) {
            marks.push((format!("●{changed}"), Tone::Warn));
        }
        if let Some(unpushed) = self.unpushed.filter(|&n| n > 0) {
            marks.push((format!("↑{unpushed}"), Tone::Warn));
        }
        marks.extend(self.state_marks());
        marks.extend(self.use_marks());
        marks
    }

    fn state_marks(&self) -> Vec<(String, Tone)> {
        let mut marks = Vec::new();
        if self.merged {
            marks.push(("merged".to_string(), Tone::Good));
        } else if self.no_commits {
            marks.push(("no commits".to_string(), Tone::Plain));
        }
        if self.upstream_gone {
            marks.push(("upstream gone".to_string(), Tone::Warn));
        }
        if self.lock.is_some() {
            marks.push(("locked".to_string(), Tone::Warn));
        }
        marks
    }

    fn use_marks(&self) -> Vec<(String, Tone)> {
        let mut marks = Vec::new();
        match self.running.as_slice() {
            [] => {}
            [one] => marks.push((one.clone(), Tone::Warn)),
            many => marks.push((format!("{} running", many.len()), Tone::Warn)),
        }
        if !self.outside.is_empty() {
            marks.push((format!("{} outside", self.outside.len()), Tone::Warn));
        }
        if self.workspace.is_some() {
            marks.push(("open".to_string(), Tone::Plain));
        }
        if self.removing {
            marks.push(("removing…".to_string(), Tone::Plain));
        }
        marks
    }

    pub(super) fn size_label(&self, measuring: bool) -> String {
        match (&self.size, &self.size_error) {
            (Some(size), _) if size.builds_bytes > 0 => {
                format!("{} · {} builds", human_size(size.bytes), human_size(size.builds_bytes))
            }
            (Some(size), _) => human_size(size.bytes),
            (None, Some(_)) => "?".into(),
            (None, None) if measuring => "…".into(),
            (None, None) => String::new(),
        }
    }

    pub(super) fn builds(&self) -> u64 {
        self.size.as_ref().map_or(0, |s| s.builds_bytes)
    }

    pub(super) fn removable(&self) -> bool {
        self.workspace.is_some() && !self.run_role && !self.removing
    }

    pub(super) fn remove_command(&self) -> Option<Vec<String>> {
        let id = self.workspace.filter(|_| self.removable())?;
        let mut command: Vec<String> =
            ["close", "--workspace", &id.to_string(), "--remove-worktree"].map(str::to_string).to_vec();
        if self.changed.is_some_and(|n| n > 0) {
            command.push("--force".into());
        }
        if self.lock.is_some() {
            command.push("--unlock".into());
        }
        Some(command)
    }
}

pub(super) fn free_command(path: &str, check: bool, force: bool) -> Vec<String> {
    let mut command: Vec<String> = ["worktrees", "free-builds", "--path", path, "--json"].map(str::to_string).to_vec();
    if check {
        command.push("--check".into());
    }
    if force {
        command.push("--force".into());
    }
    command
}

pub(super) fn list_command(sizes: bool) -> Vec<String> {
    let mut command: Vec<String> = ["worktrees", "list", "--json"].map(str::to_string).to_vec();
    if !sizes {
        command.push("--no-sizes".into());
    }
    command
}

pub(super) fn shown(worktrees: &[Worktree], filter: &str, order: Order) -> Vec<usize> {
    let mut shown: Vec<usize> = (0..worktrees.len()).filter(|&i| worktrees[i].matches(filter)).collect();
    if order == Order::LargestFirst {
        shown.sort_by_key(|&i| std::cmp::Reverse(worktrees[i].size.as_ref().map(|s| s.bytes)));
    }
    shown
}

pub(super) fn total(worktrees: &[Worktree], now: i64) -> String {
    let bytes: u64 = worktrees.iter().filter_map(|w| w.size.as_ref()).map(|s| s.bytes).sum();
    let idle: u64 = worktrees.iter().filter(|w| w.idle(now)).map(Worktree::builds).sum();
    let unknown = worktrees.iter().any(|w| w.size.is_none());
    let plus = if unknown { "+" } else { "" };
    let count = worktrees.len();
    let mut text = format!("{}{plus} in {count} worktree{}", human_size(bytes), if count == 1 { "" } else { "s" });
    if idle > 0 {
        let _ = write!(text, " · {} of builds idle {IDLE_DAYS}+ days", human_size(idle));
    }
    text
}

pub(super) fn free_message(freed: &Freed) -> String {
    let folders: Vec<String> = freed.folders.iter().map(|f| format!("{} {}", f.path, human_size(f.bytes))).collect();
    let mut text = format!(
        "Frees {}: {}. The worktree, its branch and its changes stay; files inside these folders are lost, \
         and the project's setup can rebuild them.",
        human_size(freed.bytes),
        folders.join(", ")
    );
    if !freed.running.is_empty() {
        let _ = write!(text, " {} runs in it and may still use them.", freed.running.join(", "));
    }
    text
}

pub(super) fn remove_message(worktrees: &[&Worktree]) -> String {
    let lines: Vec<String> = worktrees
        .iter()
        .map(|w| {
            let risks = if w.risks.is_empty() { "nothing is lost".to_string() } else { w.risks.join("; ") };
            format!("{}: {risks}.", w.title())
        })
        .collect();
    format!(
        "Their tabs stop and the project's teardown runs, then git removes each folder. The branches stay. {}",
        lines.join(" ")
    )
}
