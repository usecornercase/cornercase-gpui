use super::{Aim, MachinePlace, MachineSeen, ThereId, ThereProject, ThereWorkspace, with_boot};
use crate::cli::Version;
use crate::model::tree::{self, Item, Node};
use crate::model::{AgentRow, PaneInfo, Placed, Session, Tab, needs_you, status_rank};
use crate::store::args;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum OnServer<'a> {
    Here(u64),
    There(&'a str, ThereId),
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ThereAgent<'a> {
    pub seen: &'a MachineSeen,
    pub project: &'a ThereProject,
    pub workspace: &'a ThereWorkspace,
    pub tab: &'a Tab<ThereId>,
    pub pane: &'a PaneInfo<ThereId>,
}

#[derive(Debug, Clone)]
pub(crate) struct ThereRow<'a> {
    pub agent: ThereAgent<'a>,
    pub depth: usize,
    pub children: usize,
    pub hidden: Vec<ThereAgent<'a>>,
}

#[derive(Debug, Clone)]
pub(crate) enum SectionRow<'a> {
    Here(AgentRow<'a>),
    There(ThereRow<'a>),
}

const FOLD_BOOT_SINCE: Version = Version::new(0, 13, 32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PaneAim {
    pub machine: String,
    pub boot: String,
    pub pane: ThereId,
    pub collapse: bool,
}

#[derive(Clone, Copy)]
enum Listed<'a> {
    Here(Placed<'a>),
    There(ThereAgent<'a>),
}

impl<'a> Listed<'a> {
    fn item(&self) -> Item<OnServer<'a>> {
        match self {
            Self::Here((_, _, pane)) => {
                Item { key: OnServer::Here(pane.id), parent: pane.parent.map(OnServer::Here), folded: pane.collapsed }
            }
            Self::There(agent) => {
                let on = |pane| OnServer::There(agent.seen.name.as_str(), pane);
                Item { key: on(agent.pane.id), parent: agent.pane.parent.map(on), folded: agent.pane.collapsed }
            }
        }
    }
}

impl ThereAgent<'_> {
    pub(crate) fn place(&self) -> MachinePlace {
        MachinePlace::at(self.seen, self.workspace, Some(self.tab.id))
    }

    fn shown_in(&self, place: &MachinePlace) -> bool {
        place.machine == self.seen.name
            && place.boot == self.seen.boot
            && place.tab == Some(self.tab.id)
            && self.pane.active
    }
}

impl ThereRow<'_> {
    pub(crate) fn fold_aim(&self) -> PaneAim {
        let seen = self.agent.seen;
        PaneAim {
            machine: seen.name.clone(),
            boot: seen.boot.clone(),
            pane: self.agent.pane.id,
            collapse: !self.agent.pane.collapsed,
        }
    }

    pub(crate) fn lit_by(&self, place: &MachinePlace) -> bool {
        self.agent.shown_in(place) || self.hidden.iter().any(|agent| agent.shown_in(place))
    }

    pub(crate) fn attention(&self) -> Option<&str> {
        let statuses = self.hidden.iter().filter_map(|agent| agent.pane.status.as_deref());
        statuses.filter(|status| needs_you(status)).max_by_key(|status| status_rank(Some(status)))
    }
}

impl SectionRow<'_> {
    pub(crate) fn depth(&self) -> usize {
        match self {
            Self::Here(row) => row.depth,
            Self::There(row) => row.depth,
        }
    }

    pub(crate) fn workspace(&self) -> OnServer<'_> {
        match self {
            Self::Here(row) => OnServer::Here(row.workspace),
            Self::There(row) => OnServer::There(row.agent.seen.name.as_str(), row.agent.workspace.id),
        }
    }
}

impl PaneAim {
    fn verb(&self) -> &'static str {
        if self.collapse { "collapse" } else { "expand" }
    }

    pub(crate) fn key(&self) -> String {
        format!("{}:{}:{}", self.machine, self.boot, self.pane.key())
    }

    pub(crate) fn args(&self, release: Option<Version>) -> Vec<String> {
        let command = args(&[self.verb(), "--pane", &self.pane.0.to_string()]);
        if FOLD_BOOT_SINCE.reached_by(release) { with_boot(command, &self.boot) } else { command }
    }

    pub(crate) fn label(&self) -> &'static str {
        if self.collapse { "Collapse" } else { "Expand" }
    }

    pub(crate) fn running(&self) -> String {
        let doing = if self.collapse { "Collapsing" } else { "Expanding" };
        format!("{doing} the agent on {}…", self.machine)
    }

    pub(crate) fn failed(&self, reason: &str) -> String {
        format!("could not {} the agent on {}: {reason}", self.verb(), self.machine)
    }
}

pub(crate) fn sighted_pane<'a>(machines: &'a [MachineSeen], aim: &PaneAim) -> Result<&'a MachineSeen, String> {
    let name = &aim.machine;
    let seen = machines.iter().find(|m| m.name == *name).ok_or_else(|| format!("{name} is no longer listed"))?;
    if !seen.linked() {
        return Err(seen.link_note());
    }
    if seen.boot != aim.boot {
        return Err(format!("{name} restarted"));
    }
    let mut panes = seen.workspaces().flat_map(|ws| &ws.tabs).flat_map(|tab| &tab.panes);
    if panes.any(|pane| pane.id == aim.pane) { Ok(seen) } else { Err(format!("that agent is no longer on {name}")) }
}

fn agents_in<'a>(
    seen: &'a MachineSeen,
    project: &'a ThereProject,
    workspace: &'a ThereWorkspace,
) -> impl Iterator<Item = Listed<'a>> {
    let panes = workspace.tabs.iter().flat_map(|tab| tab.panes.iter().map(move |pane| (tab, pane)));
    panes
        .filter(|(_, pane)| pane.agent.is_some())
        .map(move |(tab, pane)| Listed::There(ThereAgent { seen, project, workspace, tab, pane }))
}

fn there_agents<'a>(
    projects: impl Iterator<Item = (&'a MachineSeen, &'a ThereProject)>,
    leaving: &dyn Fn(&Aim) -> bool,
) -> Vec<Listed<'a>> {
    let mut agents = Vec::new();
    for (seen, project) in projects {
        let kept = project.workspaces.iter().filter(|ws| !leaving(&seen.aim_at(ws)));
        agents.extend(kept.flat_map(|ws| agents_in(seen, project, ws)));
    }
    agents
}

fn section_row<'a>(listed: &[Listed<'a>], node: Node) -> SectionRow<'a> {
    let Node { at, depth, children, hidden } = node;
    match listed[at] {
        Listed::Here((workspace, tab, pane)) => SectionRow::Here(AgentRow { pane, workspace, tab, depth, children }),
        Listed::There(agent) => {
            let there = |at: &usize| match listed[*at] {
                Listed::There(agent) => Some(agent),
                Listed::Here(_) => None,
            };
            SectionRow::There(ThereRow { agent, depth, children, hidden: hidden.iter().filter_map(there).collect() })
        }
    }
}

impl Session {
    fn section_listed(&self, order: &[u64], leaving: &dyn Fn(&Aim) -> bool) -> Vec<Listed<'_>> {
        let mut listed = Vec::new();
        for project in order {
            listed.extend(self.placed_agents(&[*project]).into_iter().map(Listed::Here));
            listed.extend(there_agents(self.paired_projects(*project), leaving));
        }
        listed.extend(there_agents(self.alone(), leaving));
        listed
    }

    pub(crate) fn section_agents(&self, order: &[u64], leaving: impl Fn(&Aim) -> bool) -> Vec<SectionRow<'_>> {
        let listed = self.section_listed(order, &leaving);
        let items: Vec<_> = listed.iter().map(Listed::item).collect();
        tree::order(&items).into_iter().map(|node| section_row(&listed, node)).collect()
    }

    pub(crate) fn there_place(&self, agent: &ThereAgent<'_>) -> String {
        let ours = agent.project.paired.and_then(|id| self.projects.iter().find(|project| project.id == id));
        let project = ours.map_or(agent.project.name.as_str(), |project| project.name.as_str());
        let several = agent.project.workspaces.len() > 1;
        if several { format!("{project} › {}", agent.workspace.name) } else { project.to_string() }
    }
}

#[cfg(test)]
mod tests;
