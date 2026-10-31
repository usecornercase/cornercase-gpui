use serde::Deserialize;

use crate::model::Session;
use crate::panels::processes::data::command_line;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(crate) struct List {
    pub(crate) ports: Vec<Port>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(crate) struct Port {
    #[serde(rename = "port")]
    pub(crate) number: u16,
    pub(crate) addresses: Vec<String>,
    pub(crate) url: Option<String>,
    pub(crate) pid: u64,
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
    pub(crate) workspace: Option<u64>,
    pub(crate) pane: Option<u64>,
    pub(crate) via: Option<String>,
    pub(crate) agent: Option<String>,
}

impl Port {
    pub(super) fn key(&self) -> String {
        format!("{}-{}", self.number, self.pid)
    }

    pub(crate) fn command_line(&self) -> String {
        command_line(&self.program, &self.args)
    }

    pub(super) fn stoppable(&self) -> bool {
        self.workspace.is_some()
    }

    pub(super) fn stop_command(&self) -> Vec<String> {
        ["ports", "stop", &self.number.to_string(), "--pid", &self.pid.to_string()].map(str::to_string).to_vec()
    }

    pub(crate) fn by_agent(&self) -> bool {
        self.via.as_deref() == Some("agent")
    }

    pub(crate) fn place(&self, session: Option<&Session>, with_workspace: bool) -> String {
        let pane = self.pane.map(|p| (p, session.and_then(|s| s.tab_of_pane(p)).map(|t| t.name.clone())));
        let how = match (self.via.as_deref(), pane) {
            (Some("agent"), Some((p, _))) => {
                format!("agent {} · pane {p}", self.agent.as_deref().unwrap_or("?"))
            }
            (_, Some((p, Some(tab)))) => format!("pane {p} · {tab}"),
            (_, Some((p, None))) => format!("pane {p}"),
            (Some("cwd"), None) => "in the worktree, no tab".into(),
            _ => "no workspace".into(),
        };
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

pub(super) fn stop_warning(port: &Port) -> String {
    let mut text = format!(
        "{} (pid {}) gets SIGTERM, then SIGKILL if port {} is still held after 3 s. \
         Only that process is signalled; the shell or agent that started it stays.",
        port.program, port.pid, port.number
    );
    if port.by_agent() {
        text.push_str(" An agent started it, or it is the agent itself.");
    }
    if port.pane.is_none() {
        text.push_str(" No tab started it: it was found by its folder.");
    }
    text
}
