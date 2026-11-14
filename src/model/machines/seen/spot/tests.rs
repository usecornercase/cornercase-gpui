use super::*;
use crate::cli::Version;
use crate::model::Session;
use crate::test_util::{fixture, reads_with_boot, shop_on_vps};

fn session() -> Session {
    serde_json::from_str(fixture!("machines/view.json")).expect("a view line")
}

#[test]
fn asking_a_machine_names_only_its_project() {
    assert_eq!(shop_on_vps().asking_args(ReadReleases::default()), ["--project", "8"]);
}

#[test]
fn starting_on_a_machine_names_it_and_its_project() {
    assert_eq!(shop_on_vps().starting_args(), ["--machine", "vps", "--project", "8"]);
}

#[test]
fn the_login_tab_opens_in_the_workspace_at_the_projects_root() {
    let tab = LoginTab { agent: "claude", account: Some("work"), command: Some("/home/ana/bin/claude") };
    assert_eq!(
        shop_on_vps().login_tab_args(tab, None).expect("a place"),
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
    let args = shop_on_vps().login_tab_args(tab, None).expect("a place");
    assert!(args.windows(2).any(|pair| pair == ["--account", "claude="]), "{args:?}");
}

#[test]
fn an_agent_that_is_not_installed_gets_a_plain_shell() {
    let tab = LoginTab { agent: "aider", account: None, command: None };
    let args = shop_on_vps().login_tab_args(tab, None).expect("a place");
    assert_eq!(args, ["new-tab", "--workspace", "7", "--name", "aider login", "--json"]);
}

#[test]
fn a_command_the_shell_would_split_is_quoted() {
    let tab = LoginTab { agent: "claude", account: None, command: Some("/home/ana/my tools/it's claude") };
    let args = shop_on_vps().login_tab_args(tab, None).expect("a place");
    assert_eq!(args.last().map(String::as_str), Some(r"'/home/ana/my tools/it'\''s claude'"));
}

#[test]
fn a_project_with_no_workspace_there_has_no_place_for_a_login_tab() {
    let spot = Spot { root: None, ..shop_on_vps() };
    assert_eq!(spot.login_tab_args(LoginTab { agent: "claude", account: None, command: None }, None), None);
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

#[test]
fn the_login_tab_names_the_run_its_workspace_was_read_in_from_cornercase_0_13_63() {
    let tab = LoginTab { agent: "claude", account: Some("work"), command: Some("/home/ana/bin/claude") };
    for release in [Version::new(0, 13, 63), Version::new(0, 13, 68), Version::new(0, 14, 0)] {
        let args = shop_on_vps().login_tab_args(tab, Some(release)).expect("a place");
        let boot = shop_on_vps().boot;
        assert_eq!(
            args,
            [
                "new-tab",
                "--workspace",
                "7",
                "--name",
                "claude login",
                "--json",
                "--account",
                "claude=work",
                "--boot",
                boot.as_str(),
                "--",
                "/home/ana/bin/claude"
            ],
            "{release}"
        );
    }
}

#[test]
fn an_older_or_unknown_cornercase_here_opens_the_login_tab_without_the_run() {
    let tab = LoginTab { agent: "aider", account: None, command: None };
    for release in [Some(Version::new(0, 13, 62)), Some(Version::new(0, 13, 19)), None] {
        let args = shop_on_vps().login_tab_args(tab, release).expect("a place");
        assert_eq!(args, ["new-tab", "--workspace", "7", "--name", "aider login", "--json"]);
    }
}

#[test]
fn asking_a_machine_from_0_13_66_says_which_run_its_project_is_from() {
    let spot = shop_on_vps();
    let asking = spot.asking_args(reads_with_boot());
    assert_eq!(asking, ["--project", "8", "--boot", spot.boot.as_str()]);
}

#[test]
fn asking_a_machine_with_a_release_below_a_gate_or_unknown_names_only_its_project() {
    let spot = shop_on_vps();
    let release = |here, there| ReadReleases { here, there };
    let (here, there) = (Some(Version::new(0, 13, 66)), Some(Version::new(0, 12, 212)));
    for on in [release(None, there), release(here, None), release(Some(Version::new(0, 13, 65)), there)] {
        assert_eq!(spot.asking_args(on), ["--project", "8"]);
    }
    assert_eq!(spot.asking_args(release(here, Some(Version::new(0, 12, 211)))), ["--project", "8"]);
}
