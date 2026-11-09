use std::collections::BTreeMap;

use serde::Deserialize;

use crate::platform::paths;

pub(crate) mod accounts;
mod agent_context;
mod details;
mod grid;
pub(crate) mod link;
pub(crate) mod machines;
mod pull_request;
mod shared;
mod surface;
mod tree;

pub(crate) use agent_context::{Breakdown, Category, ContextAnswer, Messages, Slot};
#[cfg(test)]
pub(crate) use agent_context::{McpServer, Skills};
pub(crate) use details::memory_size;
pub(crate) use grid::{Applied, Grid, PaneGrid, Run};
pub(crate) use link::Link;
pub(crate) use machines::seen::{
    Aim, MachinePlace, MachineSeen, PaneAim, SectionRow, Spot, ThereAgent, ThereId, ThereProject, ThereRow,
    ThereWorkspace,
};
pub(crate) use pull_request::{Block, Check, Layer, PullRequest, PullRequestAnswer, Rung, Stack};
pub(crate) use shared::Shared;
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
    pub machines: Vec<MachineSeen>,
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
    pub repo: Option<String>,
    pub remotes: Vec<GitRemote>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct GitRemote {
    pub name: String,
    pub url: String,
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
    pub shared: Vec<Shared>,
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
#[serde(default, bound(deserialize = "Id: Deserialize<'de> + Default"))]
pub(crate) struct Tab<Id = u64> {
    pub id: Id,
    pub name: String,
    pub status: Option<String>,
    pub active: bool,
    pub panes: Vec<PaneInfo<Id>>,
    pub custom_name: Option<String>,
}

#[expect(clippy::struct_excessive_bools, reason = "mirrors the flags of a pane in cornercase's view")]
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, bound(deserialize = "Id: Deserialize<'de> + Default"))]
pub(crate) struct PaneInfo<Id = u64> {
    pub id: Id,
    pub path: Option<String>,
    pub program: Option<String>,
    pub agent: Option<String>,
    pub status: Option<String>,
    pub model: Option<String>,
    pub context: Option<u32>,
    pub active: bool,
    pub title: Option<String>,
    pub dialog: bool,
    pub parent: Option<Id>,
    pub subagents: u32,
    pub role: Option<String>,
    pub collapsed: bool,
    pub resuming: bool,
    pub at_prompt: bool,
    pub account: Option<String>,
    pub accounts: BTreeMap<String, String>,
    pub account_restart: Option<String>,
    pub conversation: Option<String>,
    pub held_sends: u32,
    pub held_from: Option<Id>,
    pub held_queued: bool,
    pub draft: bool,
    pub compacted: bool,
    pub memory: Option<u64>,
}

impl<Id> PaneInfo<Id> {
    pub(crate) fn running(&self) -> Option<String> {
        self.agent.clone().or_else(|| self.program.clone())
    }

    pub(crate) fn pending(&self) -> bool {
        self.status.as_deref() == Some("pending")
    }
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Shown {
    pub project: Option<u64>,
    pub workspace: Option<u64>,
    pub tab: Option<u64>,
    pub pane: Option<u64>,
}

impl Session {
    pub(crate) fn path_style(&self) -> paths::Style {
        let known = self.home.as_deref().or_else(|| self.projects.first().map(|p| p.path.as_str()));
        known.map_or_else(paths::Style::default, paths::Style::of_path)
    }

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

    pub(crate) fn agent_children(&self, pane: u64) -> usize {
        let rows = self
            .projects
            .iter()
            .flat_map(|p| &p.workspaces)
            .find_map(|w| w.agent_rows().into_iter().find(|r| r.pane.id == pane).map(|r| r.children));
        rows.unwrap_or(0)
    }

    pub(crate) fn workspace_agent(&self, workspace: u64) -> Option<(u64, String)> {
        let (_, ws) = self.workspace(workspace)?;
        let mut tabs: Vec<_> = ws.tabs.iter().collect();
        tabs.sort_by_key(|t| !t.active);
        tabs.iter().flat_map(|t| &t.panes).find_map(|p| Some((p.id, p.agent.clone()?)))
    }

    pub(crate) fn anyone_working(&self) -> bool {
        self.tabs().any(works) || self.machines.iter().any(MachineSeen::works)
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

impl<Id> PaneInfo<Id> {
    pub(crate) fn agent_title(&self) -> Option<String> {
        let title = self.title.as_deref()?.trim_start_matches(|c: char| !c.is_alphanumeric()).trim();
        (!title.is_empty()).then(|| title.to_string())
    }
}

pub(crate) fn works<Id>(tab: &Tab<Id>) -> bool {
    let working = |status: &Option<String>| status.as_deref() == Some("working");
    working(&tab.status) || tab.panes.iter().any(|p| working(&p.status))
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

impl<Id> Tab<Id> {
    pub(crate) fn loudest(&self) -> Option<&str> {
        let tab = self.status.as_deref();
        let panes = self.panes.iter().filter_map(|p| p.status.as_deref());
        std::iter::once(tab).flatten().chain(panes).max_by_key(|s| status_rank(Some(s)))
    }

    pub(crate) fn agent_pane(&self) -> Option<&PaneInfo<Id>> {
        self.panes.iter().find(|p| p.agent.is_some())
    }
}

pub(crate) fn loudest_of<Id>(tabs: &[Tab<Id>]) -> Option<&str> {
    tabs.iter().filter_map(Tab::loudest).max_by_key(|s| status_rank(Some(s)))
}

impl Workspace {
    pub(crate) fn loudest(&self) -> Option<&str> {
        loudest_of(&self.tabs)
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

fn agent_tree<'a>(agents: &[Placed<'a>], through_folds: bool) -> Vec<AgentRow<'a>> {
    let item = |(_, _, pane): &Placed<'a>| tree::Item {
        key: pane.id,
        parent: pane.parent,
        folded: pane.collapsed && !through_folds,
    };
    let items: Vec<_> = agents.iter().map(item).collect();
    let row = |node: tree::Node| {
        let (workspace, tab, pane) = agents[node.at];
        AgentRow { pane, workspace, tab, depth: node.depth, children: node.children }
    };
    tree::order(&items).into_iter().map(row).collect()
}

impl Workspace {
    pub(crate) fn agent_rows(&self) -> Vec<AgentRow<'_>> {
        agent_tree(&agents_of(self).collect::<Vec<_>>(), false)
    }
}

impl Session {
    fn placed_agents(&self, order: &[u64]) -> Vec<Placed<'_>> {
        order
            .iter()
            .filter_map(|id| self.projects.iter().find(|p| p.id == *id))
            .flat_map(|p| &p.workspaces)
            .flat_map(agents_of)
            .collect()
    }
}

pub(crate) fn needs_you(status: &str) -> bool {
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
