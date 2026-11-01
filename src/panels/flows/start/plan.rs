use serde::Deserialize;

use crate::panels::PanelKind;
use crate::panels::flows::model::project_args;
use crate::store::args;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct FlowStart {
    pub(crate) project: Option<u64>,
    pub(crate) projects: Option<Vec<u64>>,
    pub(crate) flow: Option<String>,
    pub(crate) issue: Option<IssueTask>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct IssueTask {
    pub(crate) key: String,
    pub(crate) source: String,
    pub(crate) title: String,
    pub(crate) branch: String,
    pub(crate) prompt: Option<String>,
}

impl IssueTask {
    pub(crate) fn workspace_name(&self) -> String {
        format!("{} {}", self.key, self.title)
    }

    pub(super) fn task(&self) -> String {
        self.prompt.as_deref().unwrap_or_default().lines().collect::<Vec<_>>().join(" ")
    }

    pub(super) fn learn(&mut self, prompt: Option<String>, typed: &str) -> Option<String> {
        self.prompt = prompt;
        let task = self.task();
        (typed.trim().is_empty() && !task.is_empty()).then_some(task)
    }

    pub(super) fn edited(&self, typed: &str) -> Option<String> {
        changed(typed, &self.task())
    }

    pub(super) fn read_args(&self, project: Option<u64>) -> Vec<String> {
        let mut command = args(&["issue", "read", &self.key, "--source", &self.source, "--json"]);
        command.extend(project_args(project));
        command
    }
}

#[derive(Deserialize)]
struct ReadPrompt {
    #[serde(default)]
    prompt: String,
}

pub(super) fn issue_prompt(out: &str) -> anyhow::Result<Option<String>> {
    let read: ReadPrompt = serde_json::from_str(out.trim())?;
    Ok(filled(&read.prompt).map(|_| read.prompt))
}

impl FlowStart {
    pub(crate) fn in_project(project: Option<u64>) -> Self {
        Self { project, ..Self::default() }
    }

    pub(super) fn choices(&self, open: &[(u64, String)]) -> Vec<(u64, String)> {
        let allowed = |id: &u64| self.projects.as_ref().is_none_or(|only| only.contains(id));
        open.iter().filter(|(id, _)| allowed(id)).cloned().collect()
    }

    pub(super) fn chosen(&self, choices: &[(u64, String)], shown: Option<u64>) -> Option<u64> {
        let listed = |id: &u64| choices.iter().any(|(choice, _)| choice == id);
        let wanted = self.project.filter(listed);
        if self.projects.is_some() {
            return wanted;
        }
        wanted.or(shown.filter(listed)).or_else(|| choices.first().map(|(id, _)| *id))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Typed {
    pub prompt: String,
    pub issue: String,
    pub branch: String,
    pub workspace: String,
    pub focus: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Task {
    Prompt(String),
    Issue { key: String, source: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Plan {
    pub flow: String,
    pub project: Option<u64>,
    pub task: Task,
    pub branch: Option<String>,
    pub workspace_name: Option<String>,
    pub focus: bool,
}

pub(super) fn plan(flow: &str, project: Option<u64>, issue: Option<&IssueTask>, typed: &Typed) -> Result<Plan, String> {
    let (task, branch, workspace_name) = match issue {
        Some(_) if project.is_none() => return Err("Choose the project the team works in.".into()),
        Some(issue) => on_issue(issue, typed),
        None => (typed_task(typed)?, filled(&typed.branch), None),
    };
    Ok(Plan { flow: flow.to_string(), project, task, branch, workspace_name, focus: typed.focus })
}

fn on_issue(issue: &IssueTask, typed: &Typed) -> (Task, Option<String>, Option<String>) {
    if let Some(prompt) = issue.edited(&typed.prompt) {
        let branch = filled(&typed.branch).or_else(|| filled(&issue.branch));
        return (Task::Prompt(prompt), branch, None);
    }
    (
        Task::Issue { key: issue.key.clone(), source: Some(issue.source.clone()) },
        changed(&typed.branch, &issue.branch),
        changed(&typed.workspace, &issue.workspace_name()),
    )
}

fn typed_task(typed: &Typed) -> Result<Task, String> {
    match (filled(&typed.prompt), filled(&typed.issue)) {
        (_, Some(key)) => Ok(Task::Issue { key, source: None }),
        (Some(prompt), None) => Ok(Task::Prompt(prompt)),
        (None, None) => Err("Say what the team should do, or give an issue.".into()),
    }
}

fn filled(text: &str) -> Option<String> {
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

fn changed(text: &str, prefilled: &str) -> Option<String> {
    filled(text).filter(|text| text != prefilled.trim())
}

fn flag(name: &str, value: Option<&String>) -> Vec<String> {
    value.map(|value| vec![name.to_string(), value.clone()]).unwrap_or_default()
}

impl Plan {
    pub(super) fn args(&self) -> Vec<String> {
        let mut command = args(&["flow", "run", &self.flow, "--json"]);
        command.extend(project_args(self.project));
        command.extend(match &self.task {
            Task::Prompt(prompt) => flag("--prompt", Some(prompt)),
            Task::Issue { key, source } => [flag("--issue", Some(key)), flag("--source", source.as_ref())].concat(),
        });
        command.extend(flag("--worktree", self.branch.as_ref()));
        command.extend(flag("--workspace-name", self.workspace_name.as_ref()));
        if self.focus {
            command.push("--focus".into());
        }
        command
    }
}

pub(super) fn hides_the_run(focus: bool, sheet: Option<PanelKind>) -> bool {
    focus && sheet.is_some_and(|kind| kind != PanelKind::Flows)
}

#[cfg(test)]
mod tests;
