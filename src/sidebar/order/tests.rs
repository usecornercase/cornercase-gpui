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
