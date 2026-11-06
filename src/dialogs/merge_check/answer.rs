use serde::Deserialize;
use serde_json::Value;

use crate::cli;
use crate::model::Shared;
use crate::store;

pub(super) const ASKED: &str = "the merge conflicts";

const LISTED: usize = 20;
const LEFT_OUT: &str = "over 1 MiB, or past 8 MiB in all";

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct Answer {
    checks: Vec<Merge>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
struct Merge {
    with: Option<u64>,
    base: Option<String>,
    conflicts: Vec<Conflict>,
    left_out: usize,
    error: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
struct Conflict {
    path: String,
    kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Reading {
    Checking(Option<String>),
    Answered(Answer),
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Line {
    Heading(String, Option<String>),
    File(String),
    Checking,
    Clean,
    Conflicts(String),
    Conflict(String, String),
    Failed(String),
    Note(String),
    Apart(String),
}

pub(super) struct Facts<'a> {
    pub project: &'a str,
    pub shared: &'a [Shared],
    pub shared_on: Option<bool>,
    pub name: &'a dyn Fn(u64) -> String,
}

pub(super) fn args(workspace: u64) -> Vec<String> {
    store::args(&["changes", "merge-check", "--workspace", &workspace.to_string(), "--json"])
}

pub(super) fn reading_of(result: anyhow::Result<String>) -> Reading {
    let text = match result {
        Ok(text) => text,
        Err(e) => return Reading::Failed(cli::reason(&e)),
    };
    match serde_json::from_str(&text) {
        Ok(answer) => Reading::Answered(answer),
        Err(e) => Reading::Failed(format!("cannot read the answer of cornercase changes merge-check: {e}")),
    }
}

pub(super) fn shared_files_on(config: &Value) -> Option<bool> {
    let settings = config.get("settings")?.as_array()?;
    let setting = settings.iter().find(|s| s.get("key").and_then(Value::as_str) == Some("shared_files"))?;
    setting.get("value")?.as_bool()
}

fn plural(count: usize, one: &str) -> String {
    format!("{count} {one}{}", if count == 1 { "" } else { "s" })
}

fn more(left: usize) -> Option<String> {
    (left > 0).then(|| format!("… and {left} more"))
}

fn verdict(merge: &Merge) -> Vec<Line> {
    if let Some(why) = &merge.error {
        return vec![Line::Failed(format!("Could not check: {why}"))];
    }
    let mut lines = if merge.conflicts.is_empty() { vec![Line::Clean] } else { conflict_lines(&merge.conflicts) };
    if merge.left_out > 0 {
        lines.push(Line::Note(format!("{} left out ({LEFT_OUT})", plural(merge.left_out, "untracked file"))));
    }
    lines
}

fn conflict_lines(conflicts: &[Conflict]) -> Vec<Line> {
    let mut lines = vec![Line::Conflicts(plural(conflicts.len(), "conflict"))];
    lines.extend(conflicts.iter().take(LISTED).map(|c| Line::Conflict(c.path.clone(), c.kind.clone())));
    lines.extend(more(conflicts.len().saturating_sub(LISTED)).map(Line::File));
    lines
}

fn headed(text: String, merge: &Merge) -> Vec<Line> {
    let mut lines = vec![Line::Heading(text, None)];
    lines.extend(verdict(merge));
    lines
}

fn into_base(base: Option<&String>) -> String {
    base.map_or_else(|| "Into the base branch".to_string(), |base| format!("Into {base}"))
}

fn base_lines(merge: &Merge) -> Vec<Line> {
    let mut lines = headed(into_base(merge.base.as_ref()), merge);
    if merge.base.is_none() {
        lines.push(Line::Note("Choose a base in the Changes panel.".into()));
    }
    lines
}

fn shared_lines(name: &str, shared: &Shared) -> Vec<Line> {
    let note = plural(shared.count, "shared file");
    let mut lines = vec![Line::Heading(format!("With {name}"), Some(note))];
    lines.extend(shared.files.iter().take(LISTED).cloned().map(Line::File));
    lines.extend(more(shared.beyond(LISTED)).map(Line::File));
    lines
}

fn nothing_shared(facts: &Facts<'_>) -> Option<Line> {
    Some(Line::Apart(if facts.shared_on? {
        format!("No other workspace of {} changes the same files.", facts.project)
    } else {
        "Shared files are off (Settings › Worktrees), so no other workspace was looked at.".to_string()
    }))
}

impl Reading {
    pub(super) fn checking(&self) -> bool {
        matches!(self, Self::Checking(_))
    }

    pub(super) fn again(&self) -> Self {
        Self::Checking(self.checks().iter().find(|m| m.with.is_none()).and_then(|m| m.base.clone()))
    }

    fn checks(&self) -> &[Merge] {
        match self {
            Self::Answered(answer) => &answer.checks,
            Self::Checking(_) | Self::Failed(_) => &[],
        }
    }

    fn base_merges(&self) -> Vec<Line> {
        match self {
            Self::Checking(base) => vec![Line::Heading(into_base(base.as_ref()), None), Line::Checking],
            Self::Failed(why) => vec![Line::Failed(why.clone())],
            Self::Answered(answer) => answer.checks.iter().filter(|m| m.with.is_none()).flat_map(base_lines).collect(),
        }
    }

    fn with_shared(&self, name: &str, shared: &Shared) -> Vec<Line> {
        let mut lines = shared_lines(name, shared);
        match self.checks().iter().find(|m| m.with == Some(shared.workspace)) {
            Some(merge) => lines.extend(verdict(merge)),
            None if self.checking() => lines.push(Line::Checking),
            None => {}
        }
        lines
    }

    fn with_others(&self, facts: &Facts<'_>) -> Vec<Line> {
        let listed = |id: u64| facts.shared.iter().any(|s| s.workspace == id);
        let others = self.checks().iter().filter_map(|m| m.with.filter(|id| !listed(*id)).map(|id| (id, m)));
        others.flat_map(|(id, merge)| headed(format!("With {}", (facts.name)(id)), merge)).collect()
    }

    pub(super) fn lines(&self, facts: &Facts<'_>) -> Vec<Line> {
        let mut lines = self.base_merges();
        lines.extend(facts.shared.iter().flat_map(|s| self.with_shared(&(facts.name)(s.workspace), s)));
        let others = self.with_others(facts);
        if facts.shared.is_empty() && others.is_empty() {
            lines.extend(nothing_shared(facts));
        }
        lines.extend(others);
        lines
    }

    pub(super) fn ask_text(&self, name: &dyn Fn(u64) -> String) -> Option<String> {
        let found: Vec<String> =
            self.checks().iter().filter(|m| !m.conflicts.is_empty()).map(|m| conflicts_said(m, name)).collect();
        (!found.is_empty()).then(|| format!("the merge check found conflicts {} ", found.join("; ")))
    }
}

fn conflicts_said(merge: &Merge, name: &dyn Fn(u64) -> String) -> String {
    let into = match (merge.with, &merge.base) {
        (Some(id), _) => format!("with {}", name(id)),
        (None, Some(base)) => format!("into {base}"),
        (None, None) => "into the base branch".to_string(),
    };
    let mut files: Vec<String> =
        merge.conflicts.iter().take(LISTED).map(|c| format!("{} ({})", c.path, c.kind)).collect();
    let left = merge.conflicts.len().saturating_sub(LISTED);
    files.extend((left > 0).then(|| format!("and {left} more")));
    format!("{into}: {}", files.join(", "))
}

#[cfg(test)]
mod tests;
