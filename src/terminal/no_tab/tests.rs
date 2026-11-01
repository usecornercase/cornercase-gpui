use super::{Empty, NoTab, empty_of, lead, new_shell_args};
use crate::model::{Project, Session, Shown, ViewInfo, Workspace};

fn showing(project: Option<u64>, workspace: Option<u64>) -> Session {
    let ws =
        Workspace { id: 2, name: "feat-login".into(), worktree: true, closed_agents: Some(2), ..Default::default() };
    let shown = Shown { project, workspace, tab: None, pane: None };
    Session {
        projects: vec![Project { id: 1, name: "shop".into(), workspaces: vec![ws], ..Default::default() }],
        view: Some(ViewInfo { shown, ..Default::default() }),
        ..Default::default()
    }
}

fn no_tab(closed: usize, removing: bool) -> NoTab {
    NoTab { name: "feat-login".into(), workspace: Some(2), closed, removing }
}

#[test]
fn a_server_without_projects_welcomes() {
    assert_eq!(empty_of(&Session::default(), |_| false), Empty::Welcome);
}

#[test]
fn a_shown_workspace_without_tabs_counts_its_closed_agents() {
    let empty = empty_of(&showing(Some(1), Some(2)), |_| false);
    assert_eq!(empty, Empty::NoTab(no_tab(2, false)));
}

#[test]
fn the_main_workspace_goes_by_its_project_s_name() {
    let mut session = showing(Some(1), Some(2));
    session.projects[0].workspaces[0].worktree = false;
    let Empty::NoTab(shown) = empty_of(&session, |_| false) else { panic!("a workspace") };
    assert_eq!(shown.name, "shop");
}

#[test]
fn a_workspace_being_removed_says_so() {
    let Empty::NoTab(shown) = empty_of(&showing(Some(1), Some(2)), |ws| ws == 2) else { panic!("a workspace") };
    assert!(shown.removing);
    assert_eq!(lead(&shown), "feat-login is being removed, with its worktree.");
}

#[test]
fn a_shown_project_without_a_workspace_names_the_project() {
    let empty = empty_of(&showing(Some(1), None), |_| false);
    let expected = NoTab { name: "shop".into(), workspace: None, closed: 0, removing: false };
    assert_eq!(empty, Empty::NoTab(expected));
}

#[test]
fn the_lead_offers_the_closed_agents_back() {
    assert_eq!(lead(&no_tab(0, false)), "Nothing runs in feat-login. Start an agent, or open a shell in its folder.");
    assert_eq!(
        lead(&no_tab(1, false)),
        "Nothing runs in feat-login. Its closed agent can be reopened where it left off."
    );
    assert_eq!(
        lead(&no_tab(3, false)),
        "Nothing runs in feat-login. Its 3 closed agents can be reopened where they left off."
    );
}

#[test]
fn a_new_shell_opens_in_the_shown_workspace() {
    assert_eq!(new_shell_args(Some(2)), ["new-tab", "--workspace", "2", "--focus"]);
    assert_eq!(new_shell_args(None), ["new-tab", "--focus"]);
}
