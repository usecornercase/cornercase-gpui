use anyhow::anyhow;
use serde_json::json;

use super::*;
use crate::app::machine::tests::{opening, tapped};
use crate::cli;
use crate::model::PaneInfo;
use crate::test_util::fixture;

fn v(minor: u64, patch: u64) -> Version {
    Version::new(0, minor, patch)
}

fn pane(account: Option<&str>, restart: Option<&str>) -> PaneInfo {
    serde_json::from_value(json!({
        "id": 9, "agent": "claude", "account": account, "account_restart": restart, "path": "/home/ana/shop"
    }))
    .expect("a pane")
}

fn there() -> Asked<AccountsInfo> {
    Asked::answered(Ok(fixture!("machines/there_accounts.json").to_string()))
}

fn offered(thing: &SwitchThere) -> Vec<(String, bool)> {
    let SwitchThere::Offers(switches) = thing else { panic!("expected an offer, got {thing:?}") };
    switches.iter().map(|s| (s.name.clone(), s.chosen)).collect()
}

fn run() -> MachineRun {
    MachineRun { machine: "vps".into(), boot: "b1".into() }
}

#[test]
fn cornercase_0_13_30_takes_the_boot_and_0_13_29_does_not() {
    assert!(sends_boot(Some(v(13, 30))));
    assert!(sends_boot(Some(v(13, 31))));
    assert!(sends_boot(Some(v(14, 0))));
    assert!(!sends_boot(Some(v(13, 29))));
}

#[test]
fn an_unknown_release_counts_as_older() {
    assert!(!sends_boot(None));
    assert!(boot_note(None).is_some());
}

#[test]
fn the_note_says_what_this_computer_needs() {
    let note = boot_note(Some(v(13, 21))).expect("a note");
    assert_eq!(note, format!("needs cornercase 0.13.30 or newer on this {}", desktop::here()));
    assert_eq!(boot_note(Some(v(13, 30))), None);
}

#[test]
fn a_refusal_names_the_machine_and_the_release_here() {
    let here = desktop::here();
    let known = boot_refusal(MARKING, "vps", Some(v(13, 21))).expect("a refusal");
    assert_eq!(
        known,
        format!("Marking an agent on vps needs cornercase 0.13.30 or newer on this {here} (it has 0.13.21)")
    );
    let unknown = boot_refusal(MOVING, "vps", None).expect("a refusal");
    assert_eq!(unknown, format!("Moving a pane on vps needs cornercase 0.13.30 or newer on this {here}"));
    assert_eq!(boot_refusal(MOVING, "vps", Some(v(13, 31))), None);
}

#[test]
fn the_machines_accounts_read_from_its_answer() {
    let Asked::Read(info) = there() else { panic!("expected a list") };
    assert_eq!(info.providers.iter().find(|p| p.id == "claude").map(|p| p.accounts.len()), Some(2));
}

#[test]
fn an_unreadable_answer_says_why() {
    let asked: Asked<AccountsInfo> = Asked::answered(Ok("not json".into()));
    assert_eq!(asked, Asked::Failed("its answer could not be read".into()));
}

#[test]
fn a_failed_read_says_cornercase_s_reason() {
    let failed: Asked<AccountsInfo> = Asked::answered(Err(anyhow!("cornercase account: no such command")));
    assert_eq!(failed, Asked::Failed("no such command".into()));
}

#[test]
fn an_agent_whose_machine_lists_accounts_is_offered_them_on_its_own_login_too() {
    let thing = switch_there(&there(), &pane(None, None), "vps", None);
    assert_eq!(offered(&thing), [("System".into(), true), ("home".into(), false), ("work".into(), false)]);
}

#[test]
fn an_agent_on_an_account_there_is_offered_the_machines_accounts_with_its_own_checked() {
    let thing = switch_there(&there(), &pane(Some("work"), None), "vps", None);
    assert_eq!(offered(&thing), [("System".into(), false), ("home".into(), false), ("work".into(), true)]);
}

#[test]
fn an_agent_with_no_accounts_there_has_no_switch() {
    let none = Asked::Read(AccountsInfo::default());
    assert_eq!(switch_there(&none, &pane(Some("work"), None), "vps", None), SwitchThere::Nothing);
}

#[test]
fn a_switch_rests_with_what_this_computer_needs_when_older() {
    let note = boot_note(Some(v(13, 21)));
    let thing = switch_there(&there(), &pane(None, None), "vps", note.as_deref());
    assert_eq!(thing, SwitchThere::Rests(note.expect("a note")));
}

#[test]
fn a_switch_rests_while_the_accounts_are_asked_only_for_a_pane_on_an_account() {
    let asking = Asked::Asking;
    let rests = switch_there(&asking, &pane(Some("work"), None), "vps", None);
    assert_eq!(rests, SwitchThere::Rests("Asking vps for its accounts…".into()));
    assert_eq!(switch_there(&asking, &pane(None, None), "vps", None), SwitchThere::Nothing);
}

#[test]
fn a_failed_read_rests_the_switch_with_why() {
    let failed = Asked::Failed("ssh: timed out".into());
    let thing = switch_there(&failed, &pane(None, None), "vps", None);
    assert_eq!(thing, SwitchThere::Rests("Could not read vps's accounts: ssh: timed out".into()));
}

#[test]
fn a_restart_waiting_there_offers_stay_on_it() {
    let thing = switch_there(&there(), &pane(Some("work"), Some("waiting")), "vps", None);
    let SwitchThere::Offers(switches) = thing else { panic!("expected an offer") };
    let labels: Vec<String> = switches.iter().map(Switch::label).collect();
    assert_eq!(labels, ["System", "home", "Stay on work"]);
}

#[test]
fn an_act_there_goes_on_the_machines_own_queue_with_the_tabs_boot() {
    let (tab, tap) = tapped();
    tab.send_on_run(args(&["mark", "--pane", "9"]), |_| {});
    tab.send_on_run(args(&["move-pane", "--pane", "9", "--target", "4", "--place", "left"]), |_| {});
    assert_eq!(
        tap.commands(),
        [
            vec!["mark", "--pane", "9", "--boot", "b1"],
            vec!["move-pane", "--pane", "9", "--target", "4", "--place", "left", "--boot", "b1"],
        ]
    );
}

#[test]
fn a_switch_there_says_which_run_its_pane_is_from() {
    let Asked::Read(info) = there() else { panic!("expected a list") };
    let switches = info.switches_of(&pane(Some("work"), None), None);
    let own = switches.iter().find(|s| s.folder.is_empty()).expect("the own login");
    assert_eq!(
        switch_command(own, &run()),
        ["account", "restart", "--json", "--pane", "9", "--system", "--boot", "b1", "--", "claude"]
    );
    let home = switches.iter().find(|s| s.folder == "home").expect("home");
    assert_eq!(
        switch_command(home, &run()),
        ["account", "restart", "--json", "--pane", "9", "--boot", "b1", "--", "claude", "home"]
    );
}

#[test]
fn the_switch_job_never_matches_a_local_panes() {
    assert_ne!(switch_key(&run(), 9), crate::model::accounts::switching_key(9));
    assert_eq!(switch_key(&run(), 9), "account-restart:vps:b1:9");
}

#[test]
fn an_answer_there_says_the_machine_restarted_for_a_stale_boot() {
    let stale = Err(cli::failure("mark", Some(4), b"this request is for another run"));
    let (text, kind) = there_answer("vps", &stale, None).expect("an answer");
    assert_eq!(text, "vps restarted: nothing was sent");
    assert_eq!(kind, ToastKind::Error);
}

#[test]
fn an_answer_there_names_the_machine_for_any_other_failure() {
    let failed = Err(cli::failure("mark", Some(1), b"no such pane"));
    let (text, kind) = there_answer("vps", &failed, None).expect("an answer");
    assert!(text.starts_with("vps: "), "{text}");
    assert_eq!(kind, ToastKind::Error);
}

#[test]
fn a_mark_there_is_told_once_taken_and_a_menu_mark_says_nothing() {
    let ok = Ok(String::new());
    let (text, kind) = there_answer("vps", &ok, Some("Marked as reviewed")).expect("told");
    assert_eq!((text.as_str(), kind == ToastKind::Info), ("Marked as reviewed", true));
    assert!(there_answer("vps", &ok, None).is_none());
}

#[test]
fn a_new_connection_reads_the_accounts_again() {
    let mut tab = opening();
    tab.accounts = Asked::Failed("x".into());
    let (tx, _rx) = futures::channel::mpsc::unbounded();
    tab.reopen(cli::Target::Local, &opening().aimed, &tx);
    assert_eq!(tab.accounts, Asked::Unasked);
}

#[test]
fn only_a_failed_read_of_the_accounts_is_tried_again_when_a_menu_opens() {
    let mut tab = opening();
    assert!(!tab.accounts_failed());
    tab.accounts = Asked::Failed("ssh: timed out".into());
    assert!(tab.accounts_failed());
    tab.accounts = there();
    assert!(!tab.accounts_failed());
}

#[test]
fn an_older_read_of_the_accounts_never_replaces_a_newer_ask() {
    let mut tab = opening();
    let serial = tab.conn.queue_serial();
    let (_, first) = tab.ask_accounts();
    let (_, second) = tab.ask_accounts();
    let list = || Ok(fixture!("machines/there_accounts.json").to_string());
    assert!(!tab.take_accounts(serial, first, list()));
    assert_eq!(tab.accounts, Asked::Asking);
    assert!(tab.take_accounts(serial, second, list()));
    assert!(matches!(tab.accounts, Asked::Read(_)));
    assert!(!tab.take_accounts(serial, first, Err(anyhow!("late"))));
    assert!(matches!(tab.accounts, Asked::Read(_)));
}
