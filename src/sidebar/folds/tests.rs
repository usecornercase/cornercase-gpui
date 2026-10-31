use super::*;
use crate::model::{Group, PaneInfo, Project, Tab, Workspace};

fn session(collapsed: bool) -> Session {
    let pane = PaneInfo { id: 40, collapsed, ..Default::default() };
    let tab = Tab { id: 30, panes: vec![pane], ..Default::default() };
    let workspace = Workspace { id: 20, collapsed, tabs: vec![tab], ..Default::default() };
    Session {
        groups: vec![Group { id: 1, collapsed, ..Default::default() }],
        projects: vec![Project { id: 10, collapsed, workspaces: vec![workspace], ..Default::default() }],
        ..Default::default()
    }
}

fn project_folded(session: &Session) -> bool {
    session.projects[0].collapsed
}

#[test]
fn a_fold_shows_at_once_in_the_session() {
    let mut folds = Folds::default();
    let mut shown = session(false);
    for fold in [Fold::Group(1), Fold::Project(10), Fold::Workspace(20), Fold::Pane(40)] {
        folds.set(fold, true, Some(&mut shown));
    }
    let workspace = &shown.projects[0].workspaces[0];
    assert!(shown.groups[0].collapsed && project_folded(&shown) && workspace.collapsed);
    assert!(workspace.tabs[0].panes[0].collapsed);
}

#[test]
fn a_session_from_before_the_fold_does_not_undo_it() {
    let mut folds = Folds::default();
    folds.set(Fold::Project(10), true, None);
    let mut stale = session(false);
    folds.apply(&mut stale);
    assert!(project_folded(&stale));
}

#[test]
fn a_session_that_shows_the_fold_ends_it() {
    let mut folds = Folds::default();
    folds.set(Fold::Project(10), true, None);
    folds.apply(&mut session(true));
    let mut later = session(false);
    folds.apply(&mut later);
    assert!(!project_folded(&later));
}

#[test]
fn after_the_answer_the_next_session_has_the_last_word() {
    let mut folds = Folds::default();
    folds.set(Fold::Project(10), true, None);
    folds.answered(Fold::Project(10), true, true, None);
    let mut unfolded_elsewhere = session(false);
    folds.apply(&mut unfolded_elsewhere);
    assert!(!project_folded(&unfolded_elsewhere));
}

#[test]
fn a_failed_fold_goes_back() {
    let mut folds = Folds::default();
    let mut shown = session(false);
    folds.set(Fold::Project(10), true, Some(&mut shown));
    folds.answered(Fold::Project(10), true, false, Some(&mut shown));
    assert!(!project_folded(&shown));
    let mut stale = session(false);
    folds.apply(&mut stale);
    assert!(!project_folded(&stale));
}

#[test]
fn the_answer_to_an_earlier_click_leaves_the_last_one_pending() {
    let mut folds = Folds::default();
    folds.set(Fold::Project(10), true, None);
    folds.set(Fold::Project(10), false, None);
    folds.answered(Fold::Project(10), true, true, None);
    let mut folded = session(true);
    folds.apply(&mut folded);
    assert!(!project_folded(&folded));
}

#[test]
fn a_fold_sends_the_command_for_its_kind() {
    assert_eq!(Fold::Group(1).args(true), ["collapse", "--group", "1"]);
    assert_eq!(Fold::Pane(40).args(false), ["expand", "--pane", "40"]);
}
