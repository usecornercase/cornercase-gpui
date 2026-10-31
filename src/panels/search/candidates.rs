use std::path::Path;

use super::names::{Candidate, Goto, Kind};
use crate::model::{Group, PaneInfo, Project, Session, Tab, Workspace, status_rank};

fn plural(n: usize, one: &str) -> String {
    format!("{n} {one}{}", if n == 1 { "" } else { "s" })
}

fn distinct(items: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for item in items {
        if !out.contains(&item) {
            out.push(item);
        }
    }
    out
}

pub(super) fn tilde(path: &str, home: Option<&str>) -> String {
    let home = home.map(|h| h.trim_end_matches('/')).filter(|h| !h.is_empty());
    match home.and_then(|h| path.strip_prefix(h)) {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => format!("~{rest}"),
        _ => path.to_string(),
    }
}

pub(super) fn candidates(session: &Session) -> Vec<Candidate> {
    let mut out = Vec::new();
    for (group, ids) in session.sections() {
        let group = group.and_then(|id| session.groups.iter().find(|g| g.id == id));
        let projects: Vec<&Project> =
            ids.iter().filter_map(|id| session.projects.iter().find(|p| p.id == *id)).collect();
        out.extend(group.map(|g| group_candidate(g, &projects)));
        for project in projects {
            project_candidates(project, group, session.home.as_deref(), &mut out);
        }
    }
    out
}

fn group_candidate(group: &Group, projects: &[&Project]) -> Candidate {
    let status = projects.iter().filter_map(|p| p.attention()).max_by_key(|s| status_rank(Some(s)));
    Candidate {
        kind: Kind::Group,
        goto: Goto::Group { id: group.id, first: projects.first().map(|p| p.id) },
        name: group.name.clone(),
        detail: plural(projects.len(), "project"),
        place: String::new(),
        keys: vec![group.name.clone()],
        status: status.map(String::from),
    }
}

fn project_candidates(project: &Project, group: Option<&Group>, home: Option<&str>, out: &mut Vec<Candidate>) {
    out.push(project_candidate(project, group, home));
    for ws in &project.workspaces {
        out.push(workspace_candidate(project, ws));
        for tab in &ws.tabs {
            tab_candidates(project, ws, tab, out);
        }
    }
}

fn project_candidate(project: &Project, group: Option<&Group>, home: Option<&str>) -> Candidate {
    let folder = Path::new(&project.path).file_name().map(|n| n.to_string_lossy().into_owned());
    Candidate {
        kind: Kind::Project,
        goto: Goto::Project(project.id),
        name: project.name.clone(),
        detail: tilde(&project.path, home),
        place: group.map(|g| g.name.clone()).unwrap_or_default(),
        keys: distinct(std::iter::once(project.name.clone()).chain(folder)),
        status: project.attention().map(String::from),
    }
}

fn workspace_keys(ws: &Workspace) -> Vec<String> {
    distinct(std::iter::once(ws.name.clone()).chain(ws.branch.clone().filter(|b| !b.is_empty())))
}

fn workspace_candidate(project: &Project, ws: &Workspace) -> Candidate {
    let mut keys = workspace_keys(ws);
    let mut detail: Vec<String> = keys.iter().skip(1).cloned().collect();
    if let Some(pr) = &ws.pull_request {
        let number = format!("#{}", pr.number);
        detail.push(format!("{number} {}", pr.title).trim_end().to_string());
        keys.push(number);
        keys.extend(Some(pr.title.clone()).filter(|title| !title.is_empty()));
    }
    Candidate {
        kind: Kind::Workspace,
        goto: Goto::Workspace(ws.id),
        name: if ws.worktree { format!("⎇ {}", ws.name) } else { ws.name.clone() },
        detail: detail.join(" · "),
        place: project.name.clone(),
        keys,
        status: ws.loudest().map(String::from),
    }
}

fn tab_candidates(project: &Project, ws: &Workspace, tab: &Tab, out: &mut Vec<Candidate>) {
    let place = format!("{} › {}", project.name, ws.name);
    let agents: Vec<&PaneInfo> = tab.panes.iter().filter(|p| p.agent.is_some()).collect();
    if agents.is_empty() {
        out.push(tab_candidate(tab, workspace_keys(ws), place));
        return;
    }
    out.extend(agents.into_iter().map(|pane| agent_candidate(pane, tab, workspace_keys(ws), &place)));
}

fn tab_candidate(tab: &Tab, around: Vec<String>, place: String) -> Candidate {
    let programs =
        distinct(tab.panes.iter().filter_map(|p| p.program.clone()).filter(|p| !p.is_empty() && *p != tab.name));
    let mut detail = programs.clone();
    if tab.panes.len() > 1 {
        detail.push(plural(tab.panes.len(), "pane"));
    }
    Candidate {
        kind: Kind::Tab,
        goto: Goto::Tab(tab.id),
        name: tab.name.clone(),
        detail: detail.join(" · "),
        place,
        keys: distinct(std::iter::once(tab.name.clone()).chain(programs).chain(around)),
        status: tab.loudest().map(String::from),
    }
}

fn agent_candidate(pane: &PaneInfo, tab: &Tab, around: Vec<String>, place: &str) -> Candidate {
    let name = pane.agent_title().unwrap_or_else(|| tab.name.clone());
    let about = distinct(
        [&pane.agent, &pane.role, &pane.model].into_iter().flatten().filter(|s| !s.is_empty() && **s != name).cloned(),
    );
    let in_tab = (tab.name != name && pane.agent.as_ref() != Some(&tab.name)).then(|| tab.name.clone());
    let place = in_tab.as_ref().map_or_else(|| place.to_string(), |tab| format!("{place} › {tab}"));
    Candidate {
        kind: Kind::Agent,
        goto: Goto::Agent { pane: pane.id, tab: tab.id },
        keys: distinct(std::iter::once(name.clone()).chain(in_tab).chain(about.iter().cloned()).chain(around)),
        name,
        detail: about.join(" · "),
        place,
        status: pane.status.clone(),
    }
}
