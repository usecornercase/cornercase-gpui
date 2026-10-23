use std::fmt::Write as _;

use crate::model::{Project, Session, Workspace};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Place {
    Before(u64),
    End,
}

pub(crate) fn drop_place(siblings: &[u64], dragged: u64, target: u64) -> Option<Place> {
    if dragged == target {
        return None;
    }
    let at = siblings.iter().position(|&id| id == target)?;
    match siblings.iter().position(|&id| id == dragged) {
        Some(from) if from < at => {
            Some(siblings[at + 1..].iter().find(|&&id| id != dragged).map_or(Place::End, |&id| Place::Before(id)))
        }
        _ => Some(Place::Before(target)),
    }
}

pub(crate) fn lands_after(siblings: &[u64], dragged: u64, target: u64) -> bool {
    let pos = |id| siblings.iter().position(|&x| x == id);
    matches!((pos(dragged), pos(target)), (Some(a), Some(b)) if a < b)
}

pub(crate) fn sections(session: &Session) -> Vec<(Option<u64>, Vec<u64>)> {
    let loose: Vec<u64> = session
        .projects
        .iter()
        .filter(|p| p.group.is_none_or(|g| !session.groups.iter().any(|x| x.id == g)))
        .map(|p| p.id)
        .collect();
    let mut out = vec![(None, loose)];
    for group in &session.groups {
        let ids = session.projects.iter().filter(|p| p.group == Some(group.id)).map(|p| p.id).collect();
        out.push((Some(group.id), ids));
    }
    out
}

pub(crate) fn matches(text: &str, query: &str) -> bool {
    let text = text.to_lowercase();
    query.split_whitespace().all(|word| text.contains(&word.to_lowercase()))
}

pub(crate) fn project_matches(project: &Project, query: &str) -> bool {
    matches(&format!("{} {}", project.name, project.path), query)
}

pub(crate) fn workspace_matches(ws: &Workspace, query: &str) -> bool {
    let mut text = format!("{} {}", ws.name, ws.branch.as_deref().unwrap_or_default());
    if let Some(pr) = &ws.pull_request {
        let _ = write!(text, " #{} {}", pr.number, pr.title);
    }
    for tab in &ws.tabs {
        text.push(' ');
        text.push_str(&tab.name);
        let parts = tab.panes.iter().flat_map(|pane| [&pane.agent, &pane.model, &pane.role].into_iter().flatten());
        for part in parts {
            text.push(' ');
            text.push_str(part);
        }
    }
    matches(&text, query)
}

pub(crate) fn shows_workspace(project: &Project, ws: &Workspace, query: &str) -> bool {
    query.trim().is_empty() || project_matches(project, query) || workspace_matches(ws, query)
}

pub(crate) fn shows_project(project: &Project, query: &str) -> bool {
    query.trim().is_empty() || project.workspaces.iter().any(|w| shows_workspace(project, w, query))
}

#[cfg(test)]
mod tests;
