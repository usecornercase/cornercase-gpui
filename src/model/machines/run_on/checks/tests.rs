use super::*;
use crate::test_util::{fixture, shop_on_vps};

fn info() -> AccountsInfo {
    serde_json::from_str(fixture!("machines/there_accounts.json")).expect("what the machine answered")
}

fn trackers() -> Trackers {
    serde_json::from_str(fixture!("machines/there_sources.json")).expect("what the machine answered")
}

fn all() -> Wanted {
    Wanted { accounts: true, accounts_hold: true, tracker: Some("github".into()), flows: true }
}

fn following(project: Option<u64>) -> Checks {
    let mut checks = Checks::default();
    checks.follow("boot", true, project.map(ThereId::heard));
    checks
}

fn none(_: &str) -> Option<String> {
    None
}

fn claude(picked: Option<&str>) -> Option<AgentNote> {
    let picked = picked.map(str::to_string);
    info().agent_note(&["claude".to_string()], move |_| picked.clone())
}

#[test]
fn what_a_dialog_asks_depends_on_what_it_shows() {
    let checks = following(Some(8));
    assert_eq!(checks.due(&all()), [Check::Accounts, Check::Trackers, Check::Flows]);
    assert_eq!(checks.due(&Wanted::default()), []);
    let issues = Wanted { accounts: true, tracker: Some("github".into()), ..Wanted::default() };
    assert_eq!(checks.due(&issues), [Check::Accounts, Check::Trackers]);
}

#[test]
fn a_project_the_machine_does_not_have_is_asked_only_for_its_accounts() {
    assert_eq!(following(None).due(&all()), [Check::Accounts]);
}

#[test]
fn what_is_being_asked_is_not_asked_twice() {
    let mut checks = following(Some(8));
    checks.asked(Check::Accounts);
    checks.asked(Check::Flows);
    assert_eq!(checks.due(&all()), [Check::Trackers]);
    assert_eq!(checks.asking(), [Check::Accounts, Check::Flows]);
}

#[test]
fn a_check_that_failed_is_not_asked_again_at_every_event() {
    let mut checks = following(Some(8));
    checks.heard(Check::Accounts, Err("ssh said no".into()));
    checks.follow("boot", true, Some(ThereId::heard(8)));
    assert!(!checks.due(&all()).contains(&Check::Accounts));
}

#[test]
fn pressing_start_on_a_failure_asks_it_again() {
    let mut checks = following(Some(8));
    checks.heard(Check::Accounts, Err("ssh said no".into()));
    checks.forget_failures();
    assert!(checks.due(&all()).contains(&Check::Accounts));
}

#[test]
fn a_link_that_comes_back_asks_again_what_failed() {
    let mut checks = following(Some(8));
    checks.heard(Check::Flows, Err("ssh said no".into()));
    checks.follow("boot", false, Some(ThereId::heard(8)));
    assert!(!checks.due(&all()).contains(&Check::Flows));
    checks.follow("boot", true, Some(ThereId::heard(8)));
    assert!(checks.due(&all()).contains(&Check::Flows));
}

#[test]
fn a_machine_that_restarted_is_asked_everything_again() {
    let mut checks = following(Some(8));
    checks.heard(Check::Accounts, Ok(fixture!("machines/there_accounts.json")));
    let before = checks.generation();
    checks.follow("another", true, Some(ThereId::heard(8)));
    assert!(checks.due(&all()).contains(&Check::Accounts));
    assert_ne!(checks.generation(), before);
}

#[test]
fn a_project_the_machine_lists_later_asks_its_trackers_and_flows() {
    let mut checks = following(None);
    checks.heard(Check::Accounts, Ok(fixture!("machines/there_accounts.json")));
    checks.follow("boot", true, Some(ThereId::heard(30)));
    assert_eq!(checks.due(&all()), [Check::Trackers, Check::Flows]);
}

#[test]
fn an_answer_of_an_earlier_choice_is_dropped() {
    let mut checks = following(Some(8));
    let asked = checks.generation();
    checks.restart();
    assert!(!checks.current(asked, Check::Accounts, None));
    assert!(checks.current(checks.generation(), Check::Accounts, None));
}

#[test]
fn an_answer_about_another_project_is_dropped_but_the_accounts_are_kept() {
    let mut checks = following(Some(8));
    let asked = checks.generation();
    checks.follow("boot", true, Some(ThereId::heard(30)));
    assert!(!checks.current(asked, Check::Flows, Some(ThereId::heard(8))));
    assert!(checks.current(asked, Check::Accounts, Some(ThereId::heard(8))));
}

#[test]
fn the_commands_ask_the_machines_project() {
    let spot = shop_on_vps();
    assert_eq!(Check::Accounts.args(Some(&spot)), ["account", "list", "--check", "--json"]);
    assert_eq!(Check::Trackers.args(Some(&spot)), ["issue", "sources", "--json", "--project", "8"]);
    assert_eq!(Check::Flows.args(Some(&spot)), ["flow", "list", "--json", "--project", "8"]);
}

#[test]
fn start_waits_for_a_tracker_being_checked() {
    let mut checks = following(Some(8));
    checks.asked(Check::Trackers);
    let issues = Wanted { tracker: Some("github".into()), ..Wanted::default() };
    assert_eq!(checks.hold(&issues, "vps", "shop"), Some(Hold::Waiting));
}

#[test]
fn start_goes_on_a_connected_tracker() {
    let mut checks = following(Some(8));
    checks.heard(Check::Trackers, Ok(fixture!("machines/there_sources.json")));
    let issues = Wanted { tracker: Some("github".into()), ..Wanted::default() };
    assert_eq!(checks.hold(&issues, "vps", "shop"), None);
}

#[test]
fn start_stops_on_a_tracker_the_machine_cannot_reach() {
    let mut checks = following(Some(8));
    checks.heard(Check::Trackers, Ok(fixture!("machines/there_sources.json")));
    let issues = Wanted { tracker: Some("gitlab".into()), ..Wanted::default() };
    assert_eq!(checks.hold(&issues, "vps", "shop"), Some(Hold::Why("vps has no GitLab token for shop".into())));
}

#[test]
fn a_failing_tracker_says_why_and_lets_the_start_go() {
    let mut checks = following(Some(8));
    let answer =
        r#"{"sources": [{"source": "github", "name": "GitHub", "state": "failing", "reason": "rate limited"}]}"#;
    checks.heard(Check::Trackers, Ok(answer));
    let issues = Wanted { tracker: Some("github".into()), ..Wanted::default() };
    assert_eq!(checks.hold(&issues, "vps", "shop"), None);
    assert_eq!(checks.tracker_note(&issues, "vps", "shop").as_deref(), Some("GitHub on vps: rate limited"));
}

#[test]
fn start_stops_on_a_tracker_check_that_failed() {
    let mut checks = following(Some(8));
    checks.heard(Check::Trackers, Err("ssh said no".into()));
    let issues = Wanted { tracker: Some("github".into()), ..Wanted::default() };
    assert_eq!(checks.hold(&issues, "vps", "shop"), Some(Hold::Why("ssh said no".into())));
}

#[test]
fn a_flow_waits_for_the_flows_and_the_accounts() {
    let mut checks = following(Some(8));
    let flow = Wanted { accounts: true, accounts_hold: true, tracker: None, flows: true };
    assert_eq!(checks.hold(&flow, "vps", "shop"), Some(Hold::Waiting));
    checks.heard(Check::Flows, Ok("{}"));
    assert_eq!(checks.hold(&flow, "vps", "shop"), Some(Hold::Waiting));
    checks.heard(Check::Accounts, Ok(fixture!("machines/there_accounts.json")));
    assert_eq!(checks.hold(&flow, "vps", "shop"), None);
}

#[test]
fn an_issue_does_not_wait_for_the_accounts() {
    let checks = following(Some(8));
    let issues = Wanted { accounts: true, ..Wanted::default() };
    assert_eq!(checks.hold(&issues, "vps", "shop"), None);
}

#[test]
fn a_new_workspace_holds_on_nothing() {
    assert_eq!(following(Some(8)).hold(&Wanted::default(), "vps", "shop"), None);
}

#[test]
fn accounts_that_cannot_be_parsed_say_so() {
    let mut checks = following(Some(8));
    checks.heard(Check::Accounts, Ok("[1, 2]"));
    assert_eq!(checks.accounts, Asked::Failed(UNREADABLE.to_string()));
}

#[test]
fn a_failure_keeps_one_short_line_of_what_ssh_said() {
    let long = format!("ssh: no\nsuch host {}", "x".repeat(300));
    let said = clipped(&long);
    assert!(said.starts_with("ssh: no such host x"));
    assert!(said.ends_with('…'));
    assert_eq!(said.chars().count(), LONGEST + 1);
}

#[test]
fn a_failure_names_what_could_not_be_read_and_where() {
    assert_eq!(Check::Flows.failed("vps", "ssh said no"), "Could not read its flows on vps: ssh said no");
}

#[test]
fn a_tracker_it_does_not_know_cannot_be_read() {
    assert_eq!(trackers().lacks("plane", "vps", "shop").as_deref(), Some("vps cannot read plane issues"));
}

#[test]
fn a_tracker_lacking_settings_names_them() {
    let said = trackers().lacks("jira", "vps", "shop");
    assert_eq!(said.as_deref(), Some("vps has no Jira token for shop"));
    let tracker = Tracker {
        source: "plane".into(),
        name: "Plane".into(),
        state: TrackerState::Missing,
        missing: vec!["workspace".into()],
        reason: None,
    };
    let said = Trackers { sources: vec![tracker] }.lacks("plane", "vps", "shop");
    assert_eq!(said.as_deref(), Some("vps lacks workspace for Plane in shop"));
}

#[test]
fn a_project_that_is_no_repository_there_says_so() {
    let tracker = Tracker {
        source: "github".into(),
        name: "GitHub".into(),
        state: TrackerState::Missing,
        missing: vec!["repository".into()],
        reason: None,
    };
    let said = Trackers { sources: vec![tracker] }.lacks("github", "vps", "shop");
    assert_eq!(said.as_deref(), Some("shop on vps is not a GitHub repository"));
}

#[test]
fn a_failing_tracker_gives_its_reason() {
    let tracker = Tracker {
        source: "github".into(),
        name: "GitHub".into(),
        state: TrackerState::Failing,
        reason: Some("rate limited\n".into()),
        ..Tracker::default()
    };
    let said = Trackers { sources: vec![tracker.clone()] }.lacks("github", "vps", "shop");
    assert_eq!(said.as_deref(), Some("GitHub on vps: rate limited"));
    let quiet = Tracker { reason: None, ..tracker };
    let said = Trackers { sources: vec![quiet] }.lacks("github", "vps", "shop");
    assert_eq!(said.as_deref(), Some("GitHub on vps: failing"));
}

#[test]
fn an_agent_the_machine_lacks_says_it_is_not_installed() {
    let note = info().agent_note(&["aider".to_string()], none).expect("a note");
    assert_eq!(note.state, AgentThere::NotInstalled);
    assert_eq!(note.text("vps"), "aider is not installed on vps");
    assert_eq!(note.button(), "Open a shell there");
    assert_eq!(note.account, None);
}

#[test]
fn an_agent_on_its_own_login_when_signed_out_says_so() {
    let note = claude(None).expect("a note");
    assert_eq!((note.state, note.account.as_deref()), (AgentThere::SignedOut, Some("")));
    assert_eq!(note.text("vps"), "claude is not logged in on vps");
    assert_eq!(note.button(), "Open login tab");
}

#[test]
fn the_account_picked_is_the_one_checked() {
    assert_eq!(claude(Some("home")), None);
    assert_eq!(claude(Some("work")).and_then(|note| note.account).as_deref(), Some("work"));
}

#[test]
fn the_active_account_is_the_one_checked_without_a_pick() {
    let mut info = info();
    info.providers[0].active = "home".into();
    assert_eq!(info.agent_note(&["claude".to_string()], none), None);
}

#[test]
fn an_unknown_login_says_nothing() {
    assert_eq!(info().agent_note(&["codex".to_string()], none), None);
}

#[test]
fn an_agent_the_answer_does_not_name_says_nothing() {
    assert_eq!(info().agent_note(&["brand-new".to_string()], none), None);
}

#[test]
fn a_token_set_there_wins_over_a_login_signed_out() {
    let mut info = info();
    info.tokens = vec!["ANTHROPIC_API_KEY".into()];
    assert_eq!(info.agent_note(&["claude".to_string()], none), None);
}

#[test]
fn the_first_of_the_team_with_a_problem_is_the_one_shown() {
    let team = ["codex".to_string(), "aider".to_string(), "claude".to_string()];
    assert_eq!(info().agent_note(&team, none).map(|note| note.agent).as_deref(), Some("aider"));
}

#[test]
fn the_login_runs_the_command_the_machine_knows() {
    let info = info();
    assert_eq!(info.command_of("claude"), Some("/home/ana/.local/bin/agent"));
    assert_eq!(info.command_of("aider"), None);
    assert_eq!(info.command_of("nobody"), None);
}

#[test]
fn a_blank_command_falls_back_to_the_agents_name() {
    let mut info = info();
    info.agents[0].command = "  ".into();
    assert_eq!(info.command_of("claude"), Some("claude"));
}
