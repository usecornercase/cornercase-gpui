use serde::Deserialize;

use super::{LinkState, MachineIcon, NextTry, seen_ago};
use crate::model::{PaneInfo, Project, Session, Tab, loudest_of, needs_you, status_rank, works};
use crate::store::args;

mod agents;
mod spot;

pub(crate) use agents::{PaneAim, SectionRow, ThereAgent, ThereRow, sighted_pane};
pub(crate) use spot::{LoginTab, Spot};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Deserialize)]
#[serde(transparent)]
pub(crate) struct ThereId(u64);

impl ThereId {
    pub(crate) fn heard(id: u64) -> Self {
        Self(id)
    }

    pub(crate) fn key(self) -> String {
        format!("there-{}", self.0)
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(crate) struct MachineSeen {
    pub name: String,
    pub icon: MachineIcon,
    pub state: LinkState,
    pub seen: Option<String>,
    pub retry_at: Option<String>,
    pub boot: String,
    pub projects: Vec<ThereProject>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(crate) struct ThereProject {
    pub id: ThereId,
    pub name: String,
    pub path: String,
    pub paired: Option<u64>,
    pub repo: Option<String>,
    pub workspaces: Vec<ThereWorkspace>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(crate) struct ThereWorkspace {
    pub id: ThereId,
    pub name: String,
    pub custom_name: Option<String>,
    pub path: String,
    pub branch: Option<String>,
    pub worktree: bool,
    pub closed_agents: Option<usize>,
    pub tabs: Vec<Tab<ThereId>>,
}

impl ThereWorkspace {
    pub(crate) fn loudest(&self) -> Option<&str> {
        loudest_of(&self.tabs)
    }

    pub(crate) fn active_tab(&self) -> Option<ThereId> {
        self.tabs.iter().find(|tab| tab.active).or_else(|| self.tabs.first()).map(|tab| tab.id)
    }

    pub(crate) fn given_name(&self) -> &str {
        self.custom_name.as_deref().unwrap_or(&self.name)
    }

    pub(crate) fn tab(&self, id: Option<ThereId>) -> Option<&Tab<ThereId>> {
        self.tabs.iter().find(|tab| Some(tab.id) == id)
    }

    fn has_pane(&self, pane: ThereId) -> Option<ThereId> {
        self.tabs.iter().find(|tab| tab.panes.iter().any(|p| p.id == pane)).map(|tab| tab.id)
    }
}

impl ThereProject {
    pub(crate) fn attention(&self) -> Option<&str> {
        let loudest = self.workspaces.iter().filter_map(ThereWorkspace::loudest);
        loudest.max_by_key(|status| status_rank(Some(status))).filter(|status| needs_you(status))
    }
}

impl MachineSeen {
    pub(crate) fn linked(&self) -> bool {
        self.state == LinkState::Linked
    }

    pub(crate) fn works(&self) -> bool {
        self.linked() && self.workspaces().flat_map(|ws| &ws.tabs).any(works)
    }

    pub(crate) fn next_try(&self, now: i64) -> Option<NextTry> {
        NextTry::at(self.retry_at.as_deref(), now)
    }

    pub(crate) fn tab_note_counts(&self, tab: &Tab<ThereId>, now: i64) -> bool {
        let counts = self.seen.is_some() || self.next_try(now).is_some_and(NextTry::counts);
        !self.linked() && counts && tab.agent_pane().is_some()
    }

    pub(crate) fn pane_note_counts(&self) -> bool {
        !self.linked() && self.seen.is_some()
    }

    pub(crate) fn link_note(&self) -> String {
        let name = &self.name;
        match self.state {
            LinkState::Linked => format!("{name} is linked"),
            LinkState::Unreachable => format!("{name} unreachable"),
            LinkState::Older => format!("{name} runs an older cornercase"),
            LinkState::Connecting => format!("{name} is connecting"),
        }
    }

    fn status_note(&self, status: Option<&str>, next_try: Option<NextTry>, now: i64) -> Option<String> {
        if self.linked() {
            return None;
        }
        let status = status.unwrap_or("idle").to_string();
        let parts = [Some(status), next_try.map(NextTry::words), seen_ago(self.seen.as_deref(), now)];
        Some(parts.into_iter().flatten().collect::<Vec<_>>().join(" · "))
    }

    pub(crate) fn tab_note(&self, tab: &Tab<ThereId>, now: i64) -> Option<String> {
        tab.agent_pane()?;
        self.status_note(tab.loudest(), self.next_try(now), now)
    }

    pub(crate) fn pane_note(&self, pane: &PaneInfo<ThereId>, now: i64) -> Option<String> {
        self.status_note(pane.status.as_deref(), None, now)
    }

    pub(crate) fn workspaces(&self) -> impl Iterator<Item = &ThereWorkspace> + Clone {
        self.projects.iter().flat_map(|project| &project.workspaces)
    }

    pub(crate) fn workspace(&self, id: ThereId) -> Option<&ThereWorkspace> {
        self.workspaces().find(|ws| ws.id == id)
    }

    pub(crate) fn aim_at(&self, ws: &ThereWorkspace) -> Aim {
        Aim { machine: self.name.clone(), boot: self.boot.clone(), workspace: ws.id }
    }

    pub(crate) fn place_name(&self, workspace: ThereId) -> Option<String> {
        self.projects.iter().find_map(|project| {
            let ws = project.workspaces.iter().find(|ws| ws.id == workspace)?;
            let several = project.workspaces.len() > 1;
            Some(if several { format!("{} › {}", project.name, ws.name) } else { project.name.clone() })
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Aim {
    pub machine: String,
    pub boot: String,
    pub workspace: ThereId,
}

impl Aim {
    fn id(&self) -> String {
        self.workspace.0.to_string()
    }

    pub(crate) fn key(&self) -> String {
        format!("{}:{}:{}", self.machine, self.boot, self.workspace.0)
    }

    pub(crate) fn workspace_arg(&self) -> String {
        self.id()
    }

    pub(crate) fn rename_args(&self, name: &str) -> Vec<String> {
        args(&["rename", "--workspace", &self.id(), "--boot", &self.boot, "--", name])
    }

    pub(crate) fn close_args(&self) -> Vec<String> {
        args(&["close", "--workspace", &self.id(), "--boot", &self.boot])
    }
}

pub(crate) fn with_boot(mut args: Vec<String>, boot: &str) -> Vec<String> {
    let at = args.iter().position(|a| a == "--").unwrap_or(args.len());
    args.splice(at..at, ["--boot".to_string(), boot.to_string()]);
    args
}

pub(crate) fn sighted<'a>(
    machines: &'a [MachineSeen],
    aim: &Aim,
) -> Result<(&'a MachineSeen, &'a ThereWorkspace), String> {
    let name = &aim.machine;
    let seen = machines.iter().find(|m| m.name == *name).ok_or_else(|| format!("{name} is no longer listed"))?;
    if seen.boot != aim.boot {
        return Err(format!("{name} restarted"));
    }
    let ws = seen.workspace(aim.workspace).ok_or_else(|| format!("that workspace is no longer on {name}"))?;
    Ok((seen, ws))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MachinePlace {
    pub machine: String,
    pub boot: String,
    pub workspace: ThereId,
    pub tab: Option<ThereId>,
    path: String,
    name: String,
}

impl MachinePlace {
    pub(crate) fn at(seen: &MachineSeen, ws: &ThereWorkspace, tab: Option<ThereId>) -> Self {
        let tab = tab.filter(|id| ws.tabs.iter().any(|t| t.id == *id)).or_else(|| ws.active_tab());
        Self {
            machine: seen.name.clone(),
            boot: seen.boot.clone(),
            workspace: ws.id,
            tab,
            path: ws.path.clone(),
            name: ws.name.clone(),
        }
    }

    pub(crate) fn followed(&self, machines: &[MachineSeen]) -> Option<Self> {
        let seen = machines.iter().find(|m| m.name == self.machine)?;
        let same_run = seen.boot == self.boot;
        let found = if same_run { seen.workspace(self.workspace) } else { self.by_path(seen) };
        match found {
            Some(ws) => Some(Self::at(seen, ws, self.tab.filter(|_| same_run))),
            None => (!seen.linked()).then(|| self.clone()),
        }
    }

    fn by_path<'a>(&self, seen: &'a MachineSeen) -> Option<&'a ThereWorkspace> {
        let at_path = seen.workspaces().filter(|ws| ws.path == self.path);
        at_path.clone().find(|ws| ws.name == self.name).or_else(|| at_path.clone().next())
    }

    fn shown_flags(&self) -> Vec<String> {
        match self.tab {
            Some(tab) => args(&["--tab", &tab.0.to_string()]),
            None => args(&["--workspace", &self.workspace.0.to_string()]),
        }
    }

    pub(crate) fn open_args(&self, (cols, rows): (u16, u16), hidden: bool) -> Vec<String> {
        let mut command = args(&["view", "--cols", &cols.to_string(), "--rows", &rows.to_string()]);
        command.extend(self.shown_flags());
        command.extend(hidden.then(|| "--hidden".to_string()));
        command
    }

    pub(crate) fn focus_args(&self, view: u64) -> Vec<String> {
        let mut command = args(&["focus"]);
        command.extend(self.shown_flags());
        command.extend(args(&["--view", &view.to_string()]));
        command
    }

    pub(crate) fn shows_the_same(&self, other: &Self) -> bool {
        self.same_workspace(other) && self.tab == other.tab
    }

    pub(crate) fn gone_note(&self) -> String {
        format!("{} is no longer on {}", self.name, self.machine)
    }

    pub(crate) fn aim(&self) -> Aim {
        Aim { machine: self.machine.clone(), boot: self.boot.clone(), workspace: self.workspace }
    }

    pub(crate) fn is_at(&self, seen: &MachineSeen, ws: &ThereWorkspace) -> bool {
        self.machine == seen.name && self.boot == seen.boot && self.workspace == ws.id
    }

    pub(crate) fn same_workspace(&self, other: &Self) -> bool {
        self.machine == other.machine && self.boot == other.boot && self.workspace == other.workspace
    }

    pub(crate) fn path(&self) -> &str {
        &self.path
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }
}

fn pairs_here(session: &Session, project: &ThereProject) -> bool {
    project.paired.is_some_and(|id| session.projects.iter().any(|ours| ours.id == id))
}

impl Session {
    pub(crate) fn machine(&self, name: &str) -> Option<&MachineSeen> {
        self.machines.iter().find(|machine| machine.name == name)
    }

    pub(crate) fn paired_projects(&self, project: u64) -> impl Iterator<Item = (&MachineSeen, &ThereProject)> {
        self.machines.iter().flat_map(move |seen| {
            let paired = seen.projects.iter().filter(move |there| there.paired == Some(project));
            paired.map(move |there| (seen, there))
        })
    }

    pub(crate) fn paired_with(&self, project: u64) -> impl Iterator<Item = (&MachineSeen, &ThereWorkspace)> {
        self.paired_projects(project).flat_map(|(seen, there)| there.workspaces.iter().map(move |ws| (seen, ws)))
    }

    pub(crate) fn alone(&self) -> impl Iterator<Item = (&MachineSeen, &ThereProject)> {
        self.machines.iter().flat_map(move |seen| {
            seen.projects.iter().filter(|there| !pairs_here(self, there)).map(move |there| (seen, there))
        })
    }

    pub(crate) fn workspaces_under(&self, project: &Project) -> usize {
        project.workspaces.len() + self.paired_with(project.id).count()
    }

    pub(crate) fn attention_under<'a>(&'a self, project: &'a Project) -> Option<&'a str> {
        let theirs = self.paired_with(project.id).filter_map(|(_, ws)| ws.loudest());
        let all = project.attention().into_iter().chain(theirs.filter(|status| needs_you(status)));
        all.max_by_key(|status| status_rank(Some(status)))
    }

    pub(crate) fn place_holding_panes(&self, (machine, boot): (&str, &str), panes: &[ThereId]) -> Option<MachinePlace> {
        let seen = self.machine(machine).filter(|seen| seen.boot == boot)?;
        panes.iter().find_map(|pane| {
            seen.workspaces().find_map(|ws| ws.has_pane(*pane).map(|tab| MachinePlace::at(seen, ws, Some(tab))))
        })
    }

    pub(crate) fn pair_of(&self, place: &MachinePlace) -> Option<u64> {
        let seen = self.machine(&place.machine)?;
        let there = seen.projects.iter().find(|there| there.workspaces.iter().any(|ws| ws.id == place.workspace))?;
        there.paired.filter(|_| pairs_here(self, there))
    }

    pub(crate) fn alone_of(&self, place: &MachinePlace) -> Option<(&MachineSeen, &ThereProject)> {
        self.alone().find(|(seen, there)| {
            seen.name == place.machine && there.workspaces.iter().any(|ws| ws.id == place.workspace)
        })
    }
}

#[cfg(test)]
mod tests;
