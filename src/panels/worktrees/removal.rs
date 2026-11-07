use serde::Deserialize;
use serde_json::Value;

use super::data::{Worktree, last_folder};
use crate::store::ToastKind;

const REMOVABLE: &str = "removable";
const REMOVED: &str = "removed";
const REFUSED: &str = "refused";
const FORCE: &str = "force";
const UNLOCK: &str = "unlock";
const NO_BRANCH: &str = "no branch is checked out, so none is deleted";
const NO_ANSWER: &str = "cornercase gave no answer for it";

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Answer {
    pub(super) folders: Vec<Folder>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Folder {
    pub(super) path: String,
    outcome: String,
    why: Option<String>,
    needs: Vec<String>,
    branches: Vec<String>,
    kept: Vec<String>,
    warnings: Vec<String>,
    lines: Vec<String>,
    lost: Vec<String>,
    teardown: bool,
    roles: Vec<Value>,
    worktree: Option<Worktree>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Style {
    Heading,
    Risky,
    Line,
    Lost,
    Note,
}

pub(super) type Said = (String, Style);

impl Folder {
    pub(super) fn title(&self) -> String {
        self.worktree.as_ref().map_or_else(|| last_folder(&self.path).to_string(), Worktree::title)
    }

    pub(super) fn workspace(&self) -> Option<u64> {
        self.worktree.as_ref().and_then(|w| w.workspace)
    }

    pub(super) fn goes(&self) -> bool {
        self.outcome == REMOVABLE
    }

    pub(super) fn removed(&self) -> bool {
        self.outcome == REMOVED
    }

    fn branch(&self) -> Option<&str> {
        self.worktree.as_ref().and_then(|w| w.branch.as_deref())
    }

    fn linked(&self) -> bool {
        self.worktree.as_ref().is_some_and(|w| w.kind == "linked")
    }

    fn force(&self, branches: bool) -> bool {
        self.needs.iter().any(|need| need == FORCE) || (branches && !self.lost.is_empty())
    }

    fn unlock(&self) -> bool {
        self.needs.iter().any(|need| need == UNLOCK)
    }

    fn risky(&self, branches: bool) -> bool {
        !self.goes() || self.force(branches) || self.unlock()
    }

    fn said(&self, branches: bool) -> Vec<Said> {
        let heading = if self.risky(branches) { Style::Risky } else { Style::Heading };
        let mut said = vec![(self.title(), heading)];
        said.extend(self.lines.iter().map(|line| (line.clone(), Style::Line)));
        if !self.goes() {
            said.push((format!("cannot be removed: {}", self.reason()), Style::Lost));
        } else if branches {
            said.extend((self.linked() && self.branch().is_none()).then(|| (NO_BRANCH.to_string(), Style::Line)));
            said.extend(self.lost.iter().map(|line| (line.clone(), Style::Lost)));
        }
        said
    }

    fn reason(&self) -> &str {
        self.why.as_deref().unwrap_or(NO_ANSWER)
    }

    pub(super) fn remove_command(&self, branches: bool) -> Vec<String> {
        let mut command: Vec<String> =
            ["worktrees", "remove", "--path", &self.path, "--json"].map(str::to_string).to_vec();
        if self.force(branches) {
            command.push("--force".into());
        }
        if self.unlock() {
            command.push("--unlock".into());
        }
        if branches {
            command.push("--delete-branch".into());
        }
        command
    }

    pub(super) fn answered(self, result: Result<Answer, String>) -> Self {
        let (answer, why) = match result {
            Ok(answer) => (answer.folders.into_iter().next(), None),
            Err(e) => (None, Some(e)),
        };
        let worktree = self.worktree;
        match answer {
            Some(answer) => Self { worktree: answer.worktree.or(worktree), ..answer },
            None => Self { path: self.path, outcome: "failed".into(), why, worktree, ..Self::default() },
        }
    }

    fn removal_problem(&self) -> Option<String> {
        let title = self.title();
        match self.outcome.as_str() {
            REMOVED => None,
            REFUSED if !self.needs.is_empty() => Some(format!(
                "Did not remove {title}: it changed since you were asked ({}). Remove… asks again.",
                self.lines.join("; ")
            )),
            REFUSED => Some(format!("Did not remove {title}: {}", self.reason())),
            _ => Some(format!("Could not remove {title}: {}", self.reason())),
        }
    }

    fn warning(&self, warning: &str) -> String {
        let name = self.branch().unwrap_or(&self.path);
        let teardown = warning.strip_prefix(name).and_then(|rest| rest.strip_prefix(": ")).filter(|_| self.teardown);
        teardown.map_or_else(|| warning.to_string(), |why| format!("its teardown failed: {why}"))
    }

    fn problems(&self) -> Vec<String> {
        let title = self.title();
        let notes = self.kept.iter().cloned().chain(self.warnings.iter().map(|warning| self.warning(warning)));
        self.removal_problem().into_iter().chain(notes.map(|note| format!("{title}: {note}"))).collect()
    }

    fn went(&self) -> usize {
        1 + self.roles.len()
    }
}

pub(super) fn check_command(paths: &[String]) -> Vec<String> {
    let mut command: Vec<String> = ["worktrees", "remove", "--check", "--json", "--path"].map(str::to_string).to_vec();
    command.extend_from_slice(paths);
    command
}

fn going(folders: &[Folder]) -> impl Iterator<Item = &Folder> {
    folders.iter().filter(|folder| folder.goes())
}

pub(super) fn title(folders: &[Folder]) -> String {
    let going: Vec<&Folder> = going(folders).collect();
    match going.as_slice() {
        [one] => format!("Remove the worktree {}?", one.title()),
        many => format!("Remove {} worktrees?", many.len()),
    }
}

pub(super) fn button(folders: &[Folder], branches: bool) -> &'static str {
    let force = going(folders).any(|folder| folder.force(branches));
    match (going(folders).any(Folder::unlock), force) {
        (false, false) => "Remove",
        (false, true) => "Remove anyway",
        (true, false) => "Unlock and remove",
        (true, true) => "Unlock and remove anyway",
    }
}

pub(super) fn branch_label(folders: &[Folder]) -> Option<String> {
    let branches: Vec<&str> = going(folders).filter_map(Folder::branch).collect();
    match branches.as_slice() {
        [] => None,
        [one] => Some(format!("Also delete the branch {one}")),
        _ => Some("Also delete their branches".to_string()),
    }
}

pub(super) fn said(folders: &[Folder], branches: bool) -> Vec<Said> {
    let mut said: Vec<Said> = folders.iter().flat_map(|folder| folder.said(branches)).collect();
    let open = going(folders).filter(|folder| folder.workspace().is_some()).count();
    match open {
        0 => {}
        1 => said.push(("Its workspace closes, with its tabs.".to_string(), Style::Note)),
        _ => said.push(("Their workspaces close, with their tabs.".to_string(), Style::Note)),
    }
    said
}

pub(super) fn cannot_go(folders: &[Folder]) -> String {
    let lines = folders.iter().map(|folder| format!("Cannot remove {}: {}", folder.title(), folder.reason()));
    lines.collect::<Vec<_>>().join("\n")
}

fn gone_line(folders: &[Folder]) -> Option<String> {
    let gone: Vec<&Folder> = folders.iter().filter(|folder| folder.removed()).collect();
    let count: usize = gone.iter().map(|folder| folder.went()).sum();
    let what = match gone.as_slice() {
        [] => return None,
        [one] if count == 1 => one.title(),
        _ => format!("{count} worktrees"),
    };
    let branches: Vec<&String> = gone.iter().flat_map(|folder| &folder.branches).collect();
    let also = match branches.as_slice() {
        [] => String::new(),
        [one] => format!(" and the branch {one}"),
        many => format!(" and {} branches", many.len()),
    };
    Some(format!("Removed {what}{also}"))
}

pub(super) fn summary(folders: &[Folder]) -> (String, ToastKind) {
    let problems: Vec<String> = folders.iter().flat_map(Folder::problems).collect();
    let kind = if problems.is_empty() { ToastKind::Info } else { ToastKind::Error };
    let lines: Vec<String> = gone_line(folders).into_iter().chain(problems).collect();
    (lines.join("\n"), kind)
}

#[cfg(test)]
mod tests;
