use super::*;
use crate::model::Session;
use crate::test_util::{fixture, shop_on_vps};

fn session() -> Session {
    serde_json::from_str(fixture!("machines/view.json")).expect("a view line")
}

#[test]
fn asking_a_machine_names_only_its_project() {
    assert_eq!(shop_on_vps().asking_args(), ["--project", "8"]);
}

#[test]
fn starting_on_a_machine_names_it_and_its_project() {
    assert_eq!(shop_on_vps().starting_args(), ["--machine", "vps", "--project", "8"]);
}

#[test]
fn the_login_tab_opens_in_the_workspace_at_the_projects_root() {
    let tab = LoginTab { agent: "claude", account: Some("work"), command: Some("/home/ana/bin/claude") };
    assert_eq!(
        shop_on_vps().login_tab_args(tab).expect("a place"),
        [
            "new-tab",
            "--workspace",
            "7",
            "--name",
            "claude login",
            "--json",
            "--account",
            "claude=work",
            "--",
            "/home/ana/bin/claude"
        ]
    );
}

#[test]
fn the_login_tab_of_the_own_login_names_an_empty_account() {
    let tab = LoginTab { agent: "claude", account: Some(""), command: None };
    let args = shop_on_vps().login_tab_args(tab).expect("a place");
    assert!(args.windows(2).any(|pair| pair == ["--account", "claude="]), "{args:?}");
}

#[test]
fn an_agent_that_is_not_installed_gets_a_plain_shell() {
    let tab = LoginTab { agent: "aider", account: None, command: None };
    let args = shop_on_vps().login_tab_args(tab).expect("a place");
    assert_eq!(args, ["new-tab", "--workspace", "7", "--name", "aider login", "--json"]);
}

#[test]
fn a_command_the_shell_would_split_is_quoted() {
    let tab = LoginTab { agent: "claude", account: None, command: Some("/home/ana/my tools/it's claude") };
    let args = shop_on_vps().login_tab_args(tab).expect("a place");
    assert_eq!(args.last().map(String::as_str), Some(r"'/home/ana/my tools/it'\''s claude'"));
}

#[test]
fn a_project_with_no_workspace_there_has_no_place_for_a_login_tab() {
    let spot = Spot { root: None, ..shop_on_vps() };
    assert_eq!(spot.login_tab_args(LoginTab { agent: "claude", account: None, command: None }), None);
}

#[test]
fn a_project_there_with_only_worktrees_takes_the_first() {
    let mut session = session();
    let seen = session.machines.first_mut().expect("vps");
    seen.projects[1].workspaces.retain(|ws| ws.worktree);
    let seen = session.machine("vps").expect("vps");
    let spot = seen.spot(seen.project(ThereId::heard(8)).expect("shop there"));
    assert_eq!(spot.root, Some(ThereId::heard(12)));
}

#[test]
fn a_project_pairs_by_what_the_server_says_or_by_its_repository() {
    let session = session();
    let seen = session.machine("vps").expect("vps");
    let by_pair = Project { id: 8, ..Project::default() };
    let by_repo = Project { id: 99, repo: Some("GITHUB.COM/acme/shop".into()), ..Project::default() };
    let none = Project { id: 99, repo: Some("github.com/acme/other".into()), ..Project::default() };
    assert_eq!(seen.pair_of(&by_pair).map(|there| there.id), Some(ThereId::heard(8)));
    assert_eq!(seen.pair_of(&by_repo).map(|there| there.id), Some(ThereId::heard(8)));
    assert_eq!(seen.pair_of(&none).map(|there| there.id), None);
}
