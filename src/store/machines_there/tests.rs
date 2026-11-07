use super::*;
use crate::model::machines::LinkState;

fn linked() -> MachineSeen {
    MachineSeen { name: "vps".into(), state: LinkState::Linked, ..MachineSeen::default() }
}

fn listed(destination: &str) -> Vec<Machine> {
    vec![Machine { name: "vps".into(), destination: destination.into(), ..Machine::default() }]
}

fn aim() -> Aim {
    let seen = MachineSeen { boot: "b".into(), ..linked() };
    seen.aim_at(&crate::model::ThereWorkspace::default())
}

#[test]
fn from_this_computer_a_linked_machine_is_reached_at_its_destination() {
    let target = machine_target(&Target::Local, &listed("ana@vps.example.com"), &linked()).expect("reached");
    assert_eq!(target.flags(), ["--remote", "ana@vps.example.com"]);
}

#[test]
fn a_window_on_another_server_says_the_destinations_mean_nothing_from_here() {
    let window = Target::Remote(Remote::new("devbox", "").expect("a destination"));
    let why = machine_target(&window, &listed("ana@vps.example.com"), &linked()).expect_err("refused");
    assert_eq!(why, format!("vps is linked from devbox: its address means nothing from this {}", desktop::here()));
}

#[test]
fn a_window_on_another_server_says_so_even_for_a_machine_that_is_down() {
    let window = Target::Remote(Remote::new("devbox", "").expect("a destination"));
    let down = MachineSeen { state: LinkState::Unreachable, ..linked() };
    let why = machine_target(&window, &listed("vps-host"), &down).expect_err("refused");
    assert!(why.starts_with("vps is linked from devbox"), "{why}");
}

#[test]
fn a_machine_that_is_not_linked_is_not_reached() {
    let down = MachineSeen { state: LinkState::Unreachable, ..linked() };
    let why = machine_target(&Target::Local, &listed("vps-host"), &down).expect_err("refused");
    assert_eq!(why, "vps unreachable");
}

#[test]
fn a_machine_gone_from_the_list_is_not_reached() {
    let why = machine_target(&Target::Local, &[], &linked()).expect_err("refused");
    assert_eq!(why, "vps is no longer listed");
}

#[test]
fn a_destination_ssh_would_read_as_a_flag_is_refused() {
    let why = machine_target(&Target::Local, &listed("-oProxyCommand=x"), &linked()).expect_err("refused");
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
    assert_eq!(MachineAct::Remove { force: true, unlock: false }.args(&aim), aim.remove_args(true, false));
}

#[test]
fn a_close_and_a_removal_leave_the_list_and_a_rename_does_not() {
    assert!(MachineAct::Close.leaves());
    assert!(MachineAct::Remove { force: false, unlock: false }.leaves());
    assert!(!MachineAct::Rename("x".into()).leaves());
}

#[test]
fn a_close_and_a_removal_wait_under_one_key_and_a_rename_under_its_own() {
    let aim = aim();
    let close = MachineAct::Close.job(&aim, "notes");
    let remove = MachineAct::Remove { force: false, unlock: false }.job(&aim, "notes");
    let rename = MachineAct::Rename("x".into()).job(&aim, "notes");
    assert_eq!(close.key(), remove.key());
    assert_ne!(close.key(), rename.key());
}

#[test]
fn what_an_act_did_names_the_machine() {
    let aim = aim();
    assert_eq!(MachineAct::Close.done(&aim, "notes").as_deref(), Some("Closed notes on vps"));
    assert_eq!(MachineAct::Remove { force: false, unlock: true }.done(&aim, "f").as_deref(), Some("Removed f on vps"));
    assert_eq!(MachineAct::Rename("x".into()).done(&aim, "notes"), None);
}

#[test]
fn the_home_is_read_from_what_status_says_there() {
    assert_eq!(home_in(r#"{"home": "/home/ana"}"#).as_deref(), Some("/home/ana"));
    assert_eq!(home_in(r#"{"boot": "b"}"#), None);
    assert_eq!(home_in("not json"), None);
}
