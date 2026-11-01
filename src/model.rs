use std::collections::HashSet;

use serde::Deserialize;

mod grid;
pub(crate) mod link;
mod pull_request;
mod surface;

pub(crate) use grid::{Applied, Grid, PaneGrid, Run};
pub(crate) use link::Link;
pub(crate) use pull_request::{Block, Layer, PullRequest, Rung, Stack};
pub(crate) use surface::{
    Area, Branch, Colour, Colours, Cursor, CursorShape, Frame, MouseMode, Node, OpenPopup, Style, UnderlineKind,
};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(crate) struct Session {
    pub boot: String,
    pub revision: u64,
    pub version: String,
    pub groups: Vec<Group>,
    pub projects: Vec<Project>,
    pub view: Option<ViewInfo>,
    pub config: ConfigInfo,
    pub home: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(default)]
pub(crate) struct ConfigInfo {
    pub revision: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(crate) struct Group {
    pub id: u64,
    pub name: String,
    pub collapsed: bool,
    pub icon: Option<String>,
    pub colour: Option<u8>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(crate) struct Project {
    pub id: u64,
    pub name: String,
    pub path: String,
    pub group: Option<u64>,
    pub active: bool,
    pub collapsed: bool,
    pub workspaces: Vec<Workspace>,
    pub custom_name: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(crate) struct Workspace {
    pub id: u64,
    pub name: String,
    pub path: String,
    pub branch: Option<String>,
    pub worktree: bool,
    pub active: bool,
    pub collapsed: bool,
    pub behind: u64,
    pub tabs: Vec<Tab>,
    pub custom_name: Option<String>,
    pub pull_request: Option<PullRequest>,
    pub setup: Option<String>,
    pub changes: Option<Changes>,
    pub closed_agents: Option<usize>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(crate) struct Changes {
    pub added: u64,
    pub removed: u64,
    pub files: u64,
    pub mode: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(crate) struct Tab {
    pub id: u64,
    pub name: String,
    pub status: Option<String>,
    pub active: bool,
    pub panes: Vec<PaneInfo>,
    pub custom_name: Option<String>,
}

#[expect(clippy::struct_excessive_bools, reason = "mirrors the flags of a pane in cornercase's view")]
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(crate) struct PaneInfo {
    pub id: u64,
    pub path: Option<String>,
    pub program: Option<String>,
    pub agent: Option<String>,
    pub status: Option<String>,
    pub model: Option<String>,
    pub context: Option<u32>,
    pub active: bool,
    pub title: Option<String>,
    pub dialog: bool,
    pub parent: Option<u64>,
    pub subagents: u32,
    pub role: Option<String>,
    pub collapsed: bool,
    pub resuming: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(crate) struct ViewInfo {
    pub id: u64,
    pub projection: u64,
    pub active: bool,
    pub cols: Option<u16>,
    pub rows: Option<u16>,
    pub shown: Shown,
    pub controls: bool,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(default)]
pub(crate) struct Shown {
    pub project: Option<u64>,
    pub workspace: Option<u64>,
    pub tab: Option<u64>,
    pub pane: Option<u64>,
}

impl Session {
    pub(crate) fn shown(&self) -> Shown {
        self.view.as_ref().map(|v| v.shown).unwrap_or_default()
    }

    pub(crate) fn shows_no_tab(&self, frame: Option<u64>) -> bool {
        self.view.as_ref().is_some_and(|v| v.shown.tab.is_none() && frame.is_none_or(|p| p < v.projection))
    }

    pub(crate) fn workspace(&self, id: u64) -> Option<(&Project, &Workspace)> {
        self.projects.iter().find_map(|p| p.workspaces.iter().find(|w| w.id == id).map(|w| (p, w)))
    }

    pub(crate) fn shown_runs_flow(&self) -> bool {
        self.shown().workspace.and_then(|id| self.workspace(id)).is_some_and(|(_, w)| w.runs_flow())
    }

    pub(crate) fn tabs(&self) -> impl Iterator<Item = &Tab> {
        self.projects.iter().flat_map(|p| &p.workspaces).flat_map(|w| &w.tabs)
    }

    pub(crate) fn tab(&self, id: u64) -> Option<&Tab> {
        self.tabs().find(|t| t.id == id)
    }

    pub(crate) fn tab_of_pane(&self, pane: u64) -> Option<&Tab> {
        self.tabs().find(|t| t.panes.iter().any(|p| p.id == pane))
    }

    pub(crate) fn pane(&self, id: u64) -> Option<&PaneInfo> {
        self.tabs().flat_map(|t| &t.panes).find(|p| p.id == id)
    }

    pub(crate) fn workspace_agent(&self, workspace: u64) -> Option<(u64, String)> {
        let (_, ws) = self.workspace(workspace)?;
        let mut tabs: Vec<_> = ws.tabs.iter().collect();
        tabs.sort_by_key(|t| !t.active);
        tabs.iter().flat_map(|t| &t.panes).find_map(|p| Some((p.id, p.agent.clone()?)))
    }

    pub(crate) fn anyone_working(&self) -> bool {
        let working = |status: &Option<String>| status.as_deref() == Some("working");
        self.tabs().any(|t| working(&t.status) || t.panes.iter().any(|p| working(&p.status)))
    }

    pub(crate) fn sections(&self) -> Vec<(Option<u64>, Vec<u64>)> {
        let loose: Vec<u64> = self
            .projects
            .iter()
            .filter(|p| p.group.is_none_or(|g| !self.groups.iter().any(|x| x.id == g)))
            .map(|p| p.id)
            .collect();
        let mut out = vec![(None, loose)];
        for group in &self.groups {
            let ids = self.projects.iter().filter(|p| p.group == Some(group.id)).map(|p| p.id).collect();
            out.push((Some(group.id), ids));
        }
        out
    }
}

impl PaneInfo {
    pub(crate) fn agent_title(&self) -> Option<String> {
        let title = self.title.as_deref()?.trim_start_matches(|c: char| !c.is_alphanumeric()).trim();
        (!title.is_empty()).then(|| title.to_string())
    }
}

pub(crate) fn status_rank(status: Option<&str>) -> u8 {
    match status {
        Some("waiting") => 5,
        Some("failed") => 4,
        Some("done") => 3,
        Some("pending") => 2,
        Some("working") => 1,
        _ => 0,
    }
}

impl Tab {
    pub(crate) fn loudest(&self) -> Option<&str> {
        let tab = self.status.as_deref();
        let panes = self.panes.iter().filter_map(|p| p.status.as_deref());
        std::iter::once(tab).flatten().chain(panes).max_by_key(|s| status_rank(Some(s)))
    }

    pub(crate) fn agent_pane(&self) -> Option<&PaneInfo> {
        self.panes.iter().find(|p| p.agent.is_some())
    }
}

impl Workspace {
    pub(crate) fn loudest(&self) -> Option<&str> {
        self.tabs.iter().filter_map(Tab::loudest).max_by_key(|s| status_rank(Some(s)))
    }

    pub(crate) fn runs_flow(&self) -> bool {
        self.tabs.iter().flat_map(|t| &t.panes).any(|p| p.role.is_some())
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct AgentRow<'a> {
    pub pane: &'a PaneInfo,
    pub workspace: u64,
    pub tab: u64,
    pub depth: usize,
    pub children: usize,
}

type Placed<'a> = (u64, u64, &'a PaneInfo);

fn agents_of(ws: &Workspace) -> impl Iterator<Item = Placed<'_>> {
    ws.tabs.iter().flat_map(move |t| t.panes.iter().filter(|p| p.agent.is_some()).map(move |p| (ws.id, t.id, p)))
}

fn agent_tree<'a>(agents: &[Placed<'a>]) -> Vec<AgentRow<'a>> {
    let known = |id: Option<u64>| id.filter(|id| agents.iter().any(|(_, _, p)| p.id == *id));
    let children = |id: u64| agents.iter().filter(|(_, _, p)| known(p.parent) == Some(id)).count();
    let mut rows = Vec::new();
    let mut seen = HashSet::new();
    let mut stack: Vec<(Placed<'a>, usize)> =
        agents.iter().rev().filter(|(_, _, p)| known(p.parent).is_none()).map(|a| (*a, 0)).collect();
    while let Some(((workspace, tab, pane), depth)) = stack.pop() {
        if !seen.insert(pane.id) {
            continue;
        }
        rows.push(AgentRow { pane, workspace, tab, depth, children: children(pane.id) });
        if pane.collapsed {
            continue;
        }
        for a in agents.iter().rev().filter(|(_, _, p)| known(p.parent) == Some(pane.id)) {
            stack.push((*a, depth + 1));
        }
    }
    rows
}

impl Workspace {
    pub(crate) fn agent_rows(&self) -> Vec<AgentRow<'_>> {
        agent_tree(&agents_of(self).collect::<Vec<_>>())
    }
}

impl Session {
    pub(crate) fn agent_rows(&self, order: &[u64]) -> Vec<AgentRow<'_>> {
        let agents: Vec<Placed<'_>> = order
            .iter()
            .filter_map(|id| self.projects.iter().find(|p| p.id == *id))
            .flat_map(|p| &p.workspaces)
            .flat_map(agents_of)
            .collect();
        agent_tree(&agents)
    }
}

fn needs_you(status: &str) -> bool {
    matches!(status, "waiting" | "failed" | "done" | "pending")
}

impl Project {
    pub(crate) fn attention(&self) -> Option<&str> {
        let loudest = self.workspaces.iter().filter_map(Workspace::loudest).max_by_key(|s| status_rank(Some(s)));
        loudest.filter(|s| needs_you(s))
    }
}

#[cfg(test)]
mod tests;
