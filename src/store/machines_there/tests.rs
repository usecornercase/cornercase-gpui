use super::*;
use crate::cli::Remote;
use crate::desktop;
use crate::model::machines::{LinkState, Machine};
use crate::test_util::fixture;

fn linked() -> MachineSeen {
    MachineSeen { name: "vps".into(), state: LinkState::Linked, ..MachineSeen::default() }
}

fn listed(destination: &str) -> Vec<Machine> {
    vec![Machine { name: "vps".into(), destination: destination.into(), ..Machine::default() }]
}

fn known(machines: &[Machine]) -> Listing<'_> {
    Listing { machines, known: true }
}

fn aim() -> Aim {
    let seen = MachineSeen { boot: "b".into(), ..linked() };
    seen.aim_at(&crate::model::ThereWorkspace::default())
}

#[test]
fn from_this_computer_a_linked_machine_is_reached_at_its_destination() {
    let target = machine_target(&Target::Local, known(&listed("ana@vps.example.com")), &linked()).expect("reached");
    assert_eq!(target.flags(), ["--remote", "ana@vps.example.com"]);
}

#[test]
fn a_window_on_another_server_says_the_destinations_mean_nothing_from_here() {
    let window = Target::Remote(Remote::new("devbox", "").expect("a destination"));
    let why = machine_target(&window, known(&listed("ana@vps.example.com")), &linked()).expect_err("refused");
    assert_eq!(why, format!("vps is linked from devbox: its address means nothing from this {}", desktop::here()));
}

#[test]
fn a_window_on_another_server_says_so_even_for_a_machine_that_is_down() {
    let window = Target::Remote(Remote::new("devbox", "").expect("a destination"));
    let down = MachineSeen { state: LinkState::Unreachable, ..linked() };
    let why = machine_target(&window, known(&listed("vps-host")), &down).expect_err("refused");
    assert!(why.starts_with("vps is linked from devbox"), "{why}");
}

#[test]
fn a_machine_that_is_not_linked_is_not_reached() {
    let down = MachineSeen { state: LinkState::Unreachable, ..linked() };
    let why = machine_target(&Target::Local, known(&listed("vps-host")), &down).expect_err("refused");
    assert_eq!(why, "vps unreachable");
}

#[test]
fn a_machine_gone_from_the_list_is_not_reached() {
    let why = machine_target(&Target::Local, known(&[]), &linked()).expect_err("refused");
    assert_eq!(why, "vps is no longer listed");
}

#[test]
fn a_machine_is_waited_for_while_the_machines_list_is_not_read_yet() {
    let unread = Listing { machines: &[], known: false };
    let why = machine_target(&Target::Local, unread, &linked()).expect_err("waiting");
    assert_eq!(why, "waiting for the machines list");
}

#[test]
fn a_destination_ssh_would_read_as_a_flag_is_refused() {
    let why = machine_target(&Target::Local, known(&listed("-oProxyCommand=x")), &linked()).expect_err("refused");
    assert_eq!(why, "vps's ssh destination cannot be used");
}

#[test]
fn what_was_refused_says_nothing_was_sent() {
    assert_eq!(sent_or_not("vps unreachable"), "vps unreachable: nothing was sent");
}

#[test]
fn a_boot_refused_there_reads_as_a_machine_that_restarted() {
    let error = cli::failure("close", Some(4), b"error: this request is for the cornercase server started as 1-2");
    assert_eq!(stale_note("vps", &error).as_deref(), Some("vps restarted: nothing was sent"));
}

#[test]
fn any_other_refusal_keeps_its_own_words() {
    let error = cli::failure("close", Some(1), b"error: no workspace 7");
    assert_eq!(stale_note("vps", &error), None);
}

#[test]
fn each_act_runs_its_command() {
    let aim = aim();
    assert_eq!(MachineAct::Close.args(&aim), aim.close_args());
    assert_eq!(MachineAct::Rename("x".into()).args(&aim), aim.rename_args("x"));
}

#[test]
fn a_close_leaves_the_list_and_a_rename_does_not() {
    assert!(MachineAct::Close.leaves());
    assert!(!MachineAct::Rename("x".into()).leaves());
}

#[test]
fn a_close_waits_under_the_leaving_key_and_a_rename_under_its_own() {
    let aim = aim();
    let close = MachineAct::Close.job(&aim, "notes");
    let rename = MachineAct::Rename("x".into()).job(&aim, "notes");
    assert_eq!(close.key(), Busy::leaving(&aim, "x").key());
    assert_ne!(close.key(), rename.key());
}

#[test]
fn what_an_act_did_names_the_machine() {
    let aim = aim();
    assert_eq!(MachineAct::Close.done(&aim, "notes").as_deref(), Some("Closed notes on vps"));
    assert_eq!(MachineAct::Rename("x".into()).done(&aim, "notes"), None);
}

#[test]
fn a_failure_there_names_the_machine() {
    let error = anyhow::anyhow!("cornercase close: there is no pane 9");
    assert_eq!(failed_there("vps", &error), "vps: there is no pane 9");
}

#[test]
fn a_stale_boot_there_reads_as_a_restart() {
    let error = cli::failure("close", Some(4), b"error: this request is for the cornercase server started as 1-2");
    assert_eq!(failed_there("vps", &error), "vps restarted: nothing was sent");
}

fn pane_aim() -> PaneAim {
    PaneAim { machine: "vps".into(), boot: "1-2".into(), pane: ThereId::heard(9), collapse: true }
}

#[test]
fn a_fold_refused_for_another_run_says_the_machine_restarted() {
    let stale = cli::failure("collapse", Some(4), fixture!("machines/fold_stale.txt").as_bytes());
    assert_eq!(fold_refusal(&pane_aim(), "ana@vps-host", &stale), "vps restarted: nothing was sent");
}

#[test]
fn any_other_fold_failure_names_the_machine_and_not_its_address() {
    let refused = cli::failure("collapse", Some(1), b"error: no such pane on vps-host");
    assert_eq!(
        fold_refusal(&pane_aim(), "vps-host", &refused),
        "could not collapse the agent on vps: no such pane on vps"
    );
    let slow = anyhow::anyhow!("no answer in 15 s");
    assert_eq!(fold_refusal(&pane_aim(), "vps-host", &slow), "could not collapse the agent on vps: no answer in 15 s");
}

#[test]
fn the_latest_read_of_the_release_is_the_one_kept() {
    let mut release = CliRelease::default();
    let (first, second) = (release.ask(), release.ask());
    assert!(!release.answered(first, Some(Version::new(0, 13, 21))));
    assert_eq!(release.known, None);
    assert!(release.answered(second, Some(Version::new(0, 13, 32))));
    assert_eq!(release.known, Some(Version::new(0, 13, 32)));
}

#[test]
fn a_read_that_finds_no_release_leaves_it_unknown_and_an_equal_one_changes_nothing() {
    let mut release = CliRelease::default();
    let ask = release.ask();
    assert!(release.answered(ask, Some(Version::new(0, 13, 32))));
    let ask = release.ask();
    assert!(!release.answered(ask, Some(Version::new(0, 13, 32))));
    let ask = release.ask();
    assert!(release.answered(ask, None));
    assert_eq!(release.known, None);
}

#[test]
fn a_machines_queue_words_a_refusal_with_its_machine_and_the_windows_does_not() {
    let stale = cli::failure("held", Some(4), b"error: another run\n");
    let there = cli::Queue::on(Target::Local, ("vps".into(), Some("b1".into())), |_| {});
    assert_eq!(queue_words(&there, &stale), "vps restarted: nothing was sent");
    let here = cli::Queue::nowhere();
    assert_eq!(queue_words(&here, &stale), "cornercase held: error: another run");
    let other = cli::failure("held", Some(1), b"nothing is held");
    assert_eq!(queue_words(&there, &other), "vps: nothing is held");
    assert_eq!(queue_words(&here, &other), "cornercase held: nothing is held");
}

#[test]
fn only_a_failed_answer_of_a_machines_queue_is_worded() {
    let there = cli::Queue::on(Target::Local, ("vps".into(), None), |_| {});
    let stale = cli::failure("held", Some(4), b"error: another run\n");
    assert_eq!(answer_words(&there, Some(Err(stale))).as_deref(), Some("vps restarted: nothing was sent"));
    assert_eq!(answer_words(&there, Some(Ok(String::new()))), None);
    assert_eq!(answer_words(&there, None), None);
}

#[test]
fn a_read_refused_for_its_boot_says_the_machine_restarted_and_nothing_else_is_a_refusal() {
    let refused = cli::failure("files", Some(4), b"error: another run");
    assert_eq!(read_refusal("vps", &refused).as_deref(), Some("vps restarted"));
    assert_eq!(stale_note("vps", &refused).as_deref(), Some("vps restarted: nothing was sent"));
    assert_eq!(read_refusal("vps", &cli::failure("files", Some(1), b"no")), None);
}
