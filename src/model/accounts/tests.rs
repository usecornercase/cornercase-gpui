use super::*;
use crate::model::{Project, Tab, Workspace};
use crate::test_util::fixture;

const SHELL_ON_WORK: u64 = 3;
const AGENT_ON_WORK: u64 = 9;
const AGENT_CUSTOM: u64 = 14;
const AGENT_OWN_LOGIN: u64 = 20;
const AGENT_WAITING: u64 = 22;
const PLAIN_SHELL: u64 = 7;

fn listed() -> AccountsInfo {
    serde_json::from_str(fixture!("accounts/list.json")).expect("a list")
}

fn pane(id: u64) -> PaneInfo {
    let panes: Vec<PaneInfo> = serde_json::from_str(fixture!("accounts/panes.json")).expect("panes");
    panes.into_iter().find(|p| p.id == id).expect("the pane")
}

fn named(name: &str) -> Account {
    Account { name: name.into(), ..Default::default() }
}

fn codex_with(accounts: Vec<Account>) -> Provider {
    Provider { id: "codex".into(), title: "Codex".into(), accounts, ..Default::default() }
}

fn with_codex(accounts: Vec<Account>) -> AccountsInfo {
    let mut info = listed();
    info.providers.retain(|p| p.id != "codex");
    info.providers.push(codex_with(accounts));
    info
}

fn none_added() -> AccountsInfo {
    let mut info = listed();
    for provider in &mut info.providers {
        provider.accounts.clear();
    }
    info
}

fn switcher_of(id: u64) -> Switcher {
    listed().switchers(&pane(id)).into_iter().next().expect("a switcher")
}

fn switch_to(id: u64, folder: &str) -> Switch {
    listed().switches(&switcher_of(id)).into_iter().find(|s| s.folder == folder).expect("that switch")
}

fn answer(text: &str) -> RestartAnswer {
    serde_json::from_str(text).expect("an answer")
}

fn labels(switches: &[Switch]) -> Vec<(&str, bool)> {
    switches.iter().map(|s| (s.name.as_str(), s.chosen)).collect()
}

#[test]
fn a_pane_says_its_account_in_the_view() {
    assert_eq!(pane(AGENT_ON_WORK).account.as_deref(), Some("work"));
    assert_eq!(pane(AGENT_CUSTOM).account.as_deref(), Some("custom"));
    assert_eq!(pane(AGENT_OWN_LOGIN).account, None);
    assert_eq!(pane(SHELL_ON_WORK).accounts.get("claude").map(String::as_str), Some("work"));
    assert_eq!(pane(AGENT_WAITING).account_restart.as_deref(), Some("waiting"));
    assert!(pane(SHELL_ON_WORK).at_prompt);
}

#[test]
fn the_app_feed_gives_the_defaults_and_the_profile() {
    let app = serde_json::from_str(fixture!("app/accounts.json")).expect("the feed");
    let selection = Selection::of(&app);
    assert_eq!(selection.accounts.get("claude").map(String::as_str), Some("personal"));
    assert_eq!(selection.profile.as_deref(), Some("Home"));
}

#[test]
fn a_feed_without_accounts_selects_nothing() {
    let app = serde_json::from_str(fixture!("app/snapshot.json")).expect("the feed");
    assert_eq!(Selection::of(&app), Selection::default());
}

#[test]
fn a_server_with_no_account_shows_no_mark() {
    let info = none_added();
    assert!(!info.any());
    for id in [AGENT_ON_WORK, AGENT_CUSTOM, AGENT_OWN_LOGIN, SHELL_ON_WORK] {
        assert_eq!(info.mark(&pane(id)), None);
    }
}

#[test]
fn a_server_with_no_account_shows_no_switcher() {
    let info = none_added();
    for id in [AGENT_ON_WORK, AGENT_CUSTOM, AGENT_OWN_LOGIN, SHELL_ON_WORK, PLAIN_SHELL] {
        assert_eq!(info.switchers(&pane(id)), Vec::new());
    }
}

#[test]
fn before_the_first_list_there_is_nothing_to_show() {
    let info = AccountsInfo::default();
    assert_eq!((info.any(), info.mark(&pane(AGENT_ON_WORK))), (false, None));
}

#[test]
fn the_mark_names_an_account_by_its_label() {
    assert_eq!(listed().mark(&pane(AGENT_ON_WORK)).as_deref(), Some("Work"));
}

#[test]
fn the_mark_names_an_account_without_a_label_by_its_folder() {
    assert_eq!(listed().mark(&pane(AGENT_WAITING)).as_deref(), Some("personal"));
}

#[test]
fn the_mark_says_custom_as_such() {
    assert_eq!(listed().mark(&pane(AGENT_CUSTOM)).as_deref(), Some("custom"));
}

#[test]
fn the_mark_names_the_own_login_while_the_agent_has_accounts() {
    let mut info = listed();
    assert_eq!(info.mark(&pane(AGENT_OWN_LOGIN)).as_deref(), Some("Own"));
    info.providers[0].system_label.clear();
    assert_eq!(info.mark(&pane(AGENT_OWN_LOGIN)).as_deref(), Some("System"));
}

#[test]
fn an_agent_without_accounts_has_no_mark_beside_one_that_has_them() {
    let codex = PaneInfo { agent: Some("codex".into()), ..Default::default() };
    assert_eq!(listed().mark(&codex), None);
}

#[test]
fn a_shell_has_no_mark() {
    assert_eq!(listed().mark(&pane(SHELL_ON_WORK)), None);
}

#[test]
fn an_agent_switches_only_its_own_account() {
    let switchers = with_codex(vec![named("one")]).switchers(&pane(AGENT_ON_WORK));
    let shown: Vec<_> = switchers.iter().map(|s| (s.agent.as_str(), s.folder.as_str(), s.name.as_str())).collect();
    assert_eq!(shown, [("claude", "work", "Work")]);
    assert!(switchers[0].runs);
}

#[test]
fn a_shell_switches_one_agent_per_agent_with_accounts() {
    let info = with_codex(vec![named("one")]);
    let switchers = info.switchers(&pane(SHELL_ON_WORK));
    let shown: Vec<_> = switchers.iter().map(|s| (s.agent.as_str(), s.name.as_str(), s.runs)).collect();
    assert_eq!(shown, [("claude", "Work", false), ("codex", "System", false)]);
}

#[test]
fn a_shell_names_only_the_agents_that_have_accounts() {
    let switchers = listed().switchers(&pane(PLAIN_SHELL));
    let shown: Vec<_> = switchers.iter().map(|s| (s.agent.as_str(), s.name.as_str())).collect();
    assert_eq!(shown, [("claude", "Own")]);
}

#[test]
fn an_agent_without_accounts_has_no_switcher() {
    let codex = PaneInfo { agent: Some("codex".into()), ..Default::default() };
    let copilot = PaneInfo { agent: Some("copilot".into()), ..Default::default() };
    assert_eq!(listed().switchers(&codex), Vec::new());
    assert_eq!(listed().switchers(&copilot), Vec::new());
}

#[test]
fn the_switches_start_with_the_own_login_and_mark_the_current_account() {
    let switches = listed().switches(&switcher_of(AGENT_ON_WORK));
    assert_eq!(labels(&switches), [("Own", false), ("personal", false), ("Work", true)]);
}

#[test]
fn an_agent_on_its_own_login_has_that_one_marked() {
    let switches = listed().switches(&switcher_of(AGENT_OWN_LOGIN));
    assert_eq!(labels(&switches), [("Own", true), ("personal", false), ("Work", false)]);
}

#[test]
fn a_custom_account_shows_first_and_marked() {
    let switches = listed().switches(&switcher_of(AGENT_CUSTOM));
    assert_eq!(labels(&switches), [("custom", true), ("Own", false), ("personal", false), ("Work", false)]);
}

#[test]
fn the_current_account_does_nothing_without_a_waiting_restart() {
    let switch = switch_to(AGENT_ON_WORK, "work");
    assert!(!switch.does_anything());
    assert_eq!(switch.label(), "Work");
}

#[test]
fn the_current_account_cancels_a_waiting_restart() {
    let switch = switch_to(AGENT_WAITING, "personal");
    assert!(switch.does_anything() && switch.cancels);
    assert_eq!(switch.label(), "Stay on personal");
}

#[test]
fn another_account_restarts_the_agent() {
    assert!(switch_to(AGENT_ON_WORK, "personal").does_anything());
}

#[test]
fn a_switch_names_its_pane_agent_and_folder_after_the_options() {
    let switch = switch_to(AGENT_ON_WORK, "work");
    assert_eq!(switch.args(), ["account", "restart", "--json", "--pane", "9", "--", "claude", "work"]);
}

#[test]
fn a_switch_to_the_own_login_passes_system() {
    let switch = switch_to(AGENT_ON_WORK, "");
    assert_eq!(switch.args(), ["account", "restart", "--json", "--pane", "9", "--system", "--", "claude"]);
}

#[test]
fn a_restart_says_where_the_agent_went() {
    let said = switch_to(AGENT_ON_WORK, "personal").said(&answer(fixture!("accounts/restart-restarted.json")));
    assert_eq!(said, Said { text: "claude restarted on personal".into(), refused: false });
}

#[test]
fn a_restart_of_a_shell_says_what_it_changes() {
    let mut switch = switch_to(AGENT_ON_WORK, "personal");
    switch.runs = false;
    let said = switch.said(&answer(fixture!("accounts/restart-restarted.json")));
    assert_eq!(said.text, "A claude typed in this pane now starts on personal");
}

#[test]
fn a_waiting_restart_says_it_waits_for_the_turn() {
    let said = switch_to(AGENT_WAITING, "work").said(&answer(fixture!("accounts/restart-waiting.json")));
    assert_eq!(said, Said { text: "claude restarts on Work after this turn".into(), refused: false });
}

#[test]
fn a_skipped_restart_says_why() {
    let said = switch_to(AGENT_ON_WORK, "personal").said(&answer(fixture!("accounts/restart-skipped.json")));
    let text = "claude did not move to personal: pane 9 already uses that account";
    assert_eq!(said, Said { text: text.into(), refused: true });
}

#[test]
fn a_cancelled_restart_is_no_refusal() {
    let mut switch = switch_to(AGENT_WAITING, "personal");
    switch.pane = AGENT_ON_WORK;
    let said = switch.said(&answer(fixture!("accounts/restart-skipped.json")));
    assert_eq!(said, Said { text: "claude stays on personal".into(), refused: false });
}

#[test]
fn an_answer_about_other_panes_is_a_refusal() {
    let said = switch_to(AGENT_CUSTOM, "work").said(&answer(fixture!("accounts/restart-restarted.json")));
    assert!(said.refused);
}

#[test]
fn the_tip_says_where_the_agent_runs_and_where_new_workspaces_start() {
    let selection = Selection { accounts: BTreeMap::from([("claude".into(), "personal".into())]), profile: None };
    let tip = listed().switcher_tip(&switcher_of(AGENT_ON_WORK), &selection);
    assert_eq!(tip, "Claude Code runs on Work here\nNew workspaces start on personal");
}

#[test]
fn the_tip_of_a_shell_says_what_a_typed_agent_gets() {
    let tip = listed().switcher_tip(&switcher_of(SHELL_ON_WORK), &Selection::default());
    assert_eq!(tip, "A claude typed here starts on Work\nNew workspaces start on Own");
}

#[test]
fn the_tip_names_the_profile_in_use() {
    let selection =
        Selection { accounts: BTreeMap::from([("claude".into(), "work".into())]), profile: Some("Home".into()) };
    let tip = listed().switcher_tip(&switcher_of(AGENT_OWN_LOGIN), &selection);
    assert_eq!(tip, "Claude Code runs on Own here\nNew workspaces start on Work, from the profile Home");
}

fn session_of(panes: Vec<PaneInfo>, revision: u64) -> Session {
    let tab = Tab { panes, ..Default::default() };
    let workspace = Workspace { tabs: vec![tab], ..Default::default() };
    let project = Project { workspaces: vec![workspace], ..Default::default() };
    Session { projects: vec![project], config: super::super::ConfigInfo { revision }, ..Default::default() }
}

fn without_revision() -> Value {
    serde_json::from_str(fixture!("app/snapshot.json")).expect("the feed")
}

fn with_revision() -> Value {
    serde_json::from_str(fixture!("app/accounts-revision.json")).expect("the feed")
}

#[test]
fn the_first_line_of_the_app_stream_lists_the_accounts() {
    assert!(ListMoves::default().app_line(&without_revision()));
    assert!(ListMoves::default().app_line(&with_revision()));
}

#[test]
fn a_line_with_no_revision_after_one_with_none_lists_nothing() {
    let mut moves = ListMoves::default();
    moves.app_line(&without_revision());
    assert!(!moves.app_line(&without_revision()));
}

#[test]
fn the_revision_s_first_move_lists_again() {
    let mut moves = ListMoves::default();
    moves.app_line(&without_revision());
    assert!(moves.app_line(&with_revision()));
}

#[test]
fn a_line_that_keeps_the_revision_lists_nothing() {
    let mut moves = ListMoves::default();
    moves.app_line(&with_revision());
    assert!(!moves.app_line(&with_revision()));
}

#[test]
fn a_revision_that_moves_again_lists_again() {
    let mut moves = ListMoves::default();
    moves.app_line(&with_revision());
    let mut next = with_revision();
    next["accounts_revision"] = 2.into();
    assert!(moves.app_line(&next));
}

#[test]
fn the_view_s_first_line_leaves_the_list_to_the_app_stream() {
    assert!(!ListMoves::default().view_line(&session_of(vec![pane(AGENT_ON_WORK)], 3)));
}

#[test]
fn a_view_line_that_moves_the_config_lists_again() {
    let mut moves = ListMoves::default();
    moves.view_line(&session_of(vec![pane(AGENT_ON_WORK)], 3));
    assert!(moves.view_line(&session_of(vec![pane(AGENT_ON_WORK)], 4)));
}

#[test]
fn a_pane_on_an_account_not_seen_before_lists_nothing() {
    let mut moves = ListMoves::default();
    moves.view_line(&session_of(vec![pane(PLAIN_SHELL)], 3));
    assert!(!moves.view_line(&session_of(vec![pane(PLAIN_SHELL), pane(SHELL_ON_WORK)], 3)));
}

#[test]
fn a_switch_on_a_machine_says_whose_agent_it_restarts() {
    let switch = switch_to(AGENT_ON_WORK, "personal");
    assert_eq!(switch.doing_as("claude on vps"), "Restarting claude on vps on personal…");
    let restarted = switch.said_as(&answer(fixture!("accounts/restart-restarted.json")), "claude on vps");
    assert_eq!(restarted.text, "claude on vps restarted on personal");
    let skipped = switch.said_as(&answer(fixture!("accounts/restart-skipped.json")), "claude on vps");
    assert_eq!(skipped.text, "claude on vps did not move to personal: pane 9 already uses that account");
    let waiting =
        switch_to(AGENT_WAITING, "work").said_as(&answer(fixture!("accounts/restart-waiting.json")), "claude on vps");
    assert_eq!(waiting.text, "claude on vps restarts on Work after this turn");
}

#[test]
fn a_local_switch_still_names_only_its_agent() {
    let switch = switch_to(AGENT_ON_WORK, "personal");
    assert_eq!(switch.doing(), "Restarting claude on personal…");
    assert_eq!(switch.doing(), switch.doing_as("claude"));
}
