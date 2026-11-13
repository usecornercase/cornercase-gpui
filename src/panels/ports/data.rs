use serde::Deserialize;

use crate::model::Session;
use crate::panels::processes::data::command_line;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, bound(deserialize = "Id: Deserialize<'de> + Default"))]
pub(crate) struct List<Id = u64> {
    pub(crate) ports: Vec<Port<Id>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, bound(deserialize = "Id: Deserialize<'de> + Default"))]
pub(crate) struct Port<Id = u64> {
    #[serde(rename = "port")]
    pub(crate) number: u16,
    pub(crate) addresses: Vec<String>,
    pub(crate) url: Option<String>,
    pub(crate) pid: u64,
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
    pub(crate) workspace: Option<Id>,
    pub(crate) pane: Option<Id>,
    pub(crate) via: Option<String>,
    pub(crate) agent: Option<String>,
}

impl<Id> Port<Id> {
    pub(crate) fn command_line(&self) -> String {
        command_line(&self.program, &self.args)
    }

    pub(crate) fn stoppable(&self) -> bool {
        self.workspace.is_some()
    }

    pub(crate) fn by_agent(&self) -> bool {
        self.via.as_deref() == Some("agent")
    }

    pub(crate) fn how(&self, pane: Option<(String, Option<String>)>) -> String {
        match (self.via.as_deref(), pane) {
            (Some("agent"), Some((p, _))) => format!("agent {} · pane {p}", self.agent.as_deref().unwrap_or("?")),
            (_, Some((p, Some(tab)))) => format!("pane {p} · {tab}"),
            (_, Some((p, None))) => format!("pane {p}"),
            (Some("cwd"), None) => "in the worktree, no tab".into(),
            _ => "no workspace".into(),
        }
    }
}

impl Port {
    pub(crate) fn place(&self, session: Option<&Session>, with_workspace: bool) -> String {
        let pane = self.pane.map(|p| (p.to_string(), session.and_then(|s| s.tab_of_pane(p)).map(|t| t.name.clone())));
        let how = self.how(pane);
        let workspace = self.workspace.filter(|_| with_workspace).and_then(|id| session?.workspace(id));
        match workspace {
            Some((project, ws)) => format!("{} › {} · {how}", project.name, ws.name),
            None => how,
        }
    }
}

pub(crate) fn list_command(all: bool, workspace: Option<u64>) -> Vec<String> {
    let mut command = vec!["ports".to_string(), "--json".to_string()];
    match (all, workspace) {
        (true, _) => command.push("--all".into()),
        (false, Some(id)) => command.extend(["--workspace".to_string(), id.to_string()]),
        (false, None) => {}
    }
    command
}
