//! Where a dragged row lands, and which rows the quick filter keeps. Plain functions over ids and
//! the session, so they are tested without a window.

use crate::model::{Project, Session, Workspace};

/// Where a row dropped on a sibling goes, as `move` says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Place {
    /// `--before ID`.
    Before(u64),
    /// The end of its list: `--last`, or for a project `--into GROUP` / `--ungroup`.
    End,
}

/// A row dropped on `target`, one of `siblings` in their order: it takes the target's place,
/// so it goes after the target when it came from above (as the TUI's drag does), else before.
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

/// Whether a drop on `target` lands below it, for the line drawn while dragging.
pub(crate) fn lands_after(siblings: &[u64], dragged: u64, target: u64) -> bool {
    let pos = |id| siblings.iter().position(|&x| x == id);
    matches!((pos(dragged), pos(target)), (Some(a), Some(b)) if a < b)
}

/// The sidebar's order: projects without a group first, then each group with its projects.
/// Groups without projects are listed too, so projects can be dragged into them.
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

/// The quick filter: every word of the query, ignoring case, somewhere in the text.
pub(crate) fn matches(text: &str, query: &str) -> bool {
    let text = text.to_lowercase();
    query.split_whitespace().all(|word| text.contains(&word.to_lowercase()))
}

/// A project matches by its name or folder.
pub(crate) fn project_matches(project: &Project, query: &str) -> bool {
    matches(&format!("{} {}", project.name, project.path), query)
}

/// A workspace matches by its name, branch, pull request number, tabs or agents.
pub(crate) fn workspace_matches(ws: &Workspace, query: &str) -> bool {
    let mut text = format!("{} {}", ws.name, ws.branch.as_deref().unwrap_or_default());
    if let Some(pr) = &ws.pull_request {
        text.push_str(&format!(" #{} {}", pr.number, pr.title));
    }
    for tab in &ws.tabs {
        text.push(' ');
        text.push_str(&tab.name);
        for pane in &tab.panes {
            for part in [&pane.agent, &pane.model, &pane.role].into_iter().flatten() {
                text.push(' ');
                text.push_str(part);
            }
        }
    }
    matches(&text, query)
}

/// Whether the filter shows a workspace: its project matches (all of them show), or it does.
pub(crate) fn shows_workspace(project: &Project, ws: &Workspace, query: &str) -> bool {
    query.trim().is_empty() || project_matches(project, query) || workspace_matches(ws, query)
}

/// Whether the filter shows a project: it matches, or one of its workspaces does.
pub(crate) fn shows_project(project: &Project, query: &str) -> bool {
    query.trim().is_empty() || project.workspaces.iter().any(|w| shows_workspace(project, w, query))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Group, PaneInfo, Tab};

    #[test]
    fn a_row_dragged_down_lands_after_the_target() {
        let ids = [1, 2, 3, 4];
        assert_eq!(drop_place(&ids, 1, 3), Some(Place::Before(4)));
        assert_eq!(drop_place(&ids, 1, 4), Some(Place::End));
        assert_eq!(drop_place(&ids, 3, 4), Some(Place::End));
        assert!(lands_after(&ids, 1, 3));
    }

    #[test]
    fn a_row_dragged_up_or_from_elsewhere_lands_before() {
        let ids = [1, 2, 3];
        assert_eq!(drop_place(&ids, 3, 1), Some(Place::Before(1)));
        assert_eq!(drop_place(&ids, 9, 2), Some(Place::Before(2)));
        assert_eq!(drop_place(&ids, 2, 2), None);
        assert_eq!(drop_place(&ids, 2, 7), None);
        assert!(!lands_after(&ids, 3, 1));
        assert!(!lands_after(&ids, 9, 1));
    }

    #[test]
    fn sections_put_loose_projects_first_and_keep_empty_groups() {
        let session = Session {
            groups: vec![Group { id: 10, ..Default::default() }, Group { id: 11, ..Default::default() }],
            projects: vec![
                Project { id: 1, group: Some(10), ..Default::default() },
                Project { id: 2, ..Default::default() },
                Project { id: 3, group: Some(99), ..Default::default() },
            ],
            ..Default::default()
        };
        assert_eq!(sections(&session), vec![(None, vec![2, 3]), (Some(10), vec![1]), (Some(11), vec![])]);
    }

    #[test]
    fn the_filter_finds_projects_workspaces_and_agents() {
        let agent = PaneInfo { agent: Some("codex".into()), model: Some("gpt-5.5".into()), ..Default::default() };
        let project = Project {
            name: "shop".into(),
            path: "/src/web-shop".into(),
            workspaces: vec![
                Workspace { name: "main".into(), ..Default::default() },
                Workspace {
                    name: "feat/dark".into(),
                    tabs: vec![Tab { name: "codex".into(), panes: vec![agent], ..Default::default() }],
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert!(shows_project(&project, "WEB"));
        assert!(shows_workspace(&project, &project.workspaces[0], "shop"));
        assert!(!shows_workspace(&project, &project.workspaces[0], "dark"));
        assert!(shows_workspace(&project, &project.workspaces[1], "dark gpt"));
        assert!(!shows_project(&project, "nothing"));
        assert!(shows_project(&project, "  "));
    }
}
