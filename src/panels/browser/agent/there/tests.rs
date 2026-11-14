use std::path::PathBuf;

use super::{Hand, Stopped, hand_commands, hand_refusal, run_all};
use crate::cli::{Version, failure};
use crate::model::{PaneThere, ThereId};

const HERE: Option<Version> = Some(Version::new(0, 13, 68));

fn pane_there() -> PaneThere {
    PaneThere { machine: "vps".into(), boot: "b1".into(), pane: ThereId::heard(9) }
}

#[test]
fn the_hand_off_runs_files_then_text() {
    let hand = Hand { text: "look".into(), files: vec![PathBuf::from("/tmp/a.png")], submit: false };
    let commands = hand_commands(&pane_there(), &hand, HERE);
    assert_eq!(commands[0], ["paste-file", "--pane", "9", "/tmp/a.png", "--boot", "b1"]);
    assert_eq!(commands[1], ["input", "--pane", "9", r#"{"paste":"look"}"#, "--boot", "b1"]);
    assert_eq!(commands.len(), 2);
}

#[test]
fn a_hand_off_writes_the_machines_pane_id_and_the_boot_it_was_read_with() {
    let hand = Hand { text: "fix it".into(), files: Vec::new(), submit: true };
    let commands = hand_commands(&pane_there(), &hand, HERE);
    assert_eq!(commands, [["send", "--pane", "9", "--enter", "--boot", "b1", "--", "fix it"]]);
}

#[test]
fn nothing_to_paste_sends_only_the_files() {
    let hand = Hand { text: String::new(), files: vec![PathBuf::from("/tmp/a.png")], submit: false };
    assert_eq!(hand_commands(&pane_there(), &hand, HERE).len(), 1);
}

#[test]
fn a_hand_off_aim_is_refused_after_a_restart_or_when_the_pane_is_gone() {
    use crate::model::machines::LinkState;
    use crate::model::machines::seen::{MachineSeen, ThereProject, ThereWorkspace};
    use crate::model::{PaneInfo, Tab};
    let pane: PaneInfo<ThereId> =
        PaneInfo { id: ThereId::heard(9), agent: Some("claude".into()), ..PaneInfo::default() };
    let tab = Tab { panes: vec![pane], ..Tab::default() };
    let ws = ThereWorkspace { tabs: vec![tab], ..Default::default() };
    let project = ThereProject { workspaces: vec![ws], ..Default::default() };
    let seen = MachineSeen {
        name: "vps".into(),
        boot: "b1".into(),
        state: LinkState::Linked,
        projects: vec![project],
        ..Default::default()
    };
    assert!(pane_there().sighted(std::slice::from_ref(&seen)).is_ok());
    let restarted = MachineSeen { boot: "b2".into(), ..seen.clone() };
    assert_eq!(pane_there().sighted(&[restarted]).err().as_deref(), Some("vps restarted"));
    let other = PaneThere { pane: ThereId::heard(3), ..pane_there() };
    assert_eq!(other.sighted(&[seen]).err().as_deref(), Some("that agent is no longer on vps"));
    assert!(pane_there().sighted(&[]).is_err());
}

#[test]
fn the_hand_off_stops_at_the_first_failure() {
    let commands = vec![vec!["a".to_string()], vec!["b".to_string()], vec!["c".to_string()]];
    let mut ran = Vec::new();
    let result = run_all(&commands, |command| {
        ran.push(command[0].clone());
        if command[0] == "b" { Err(anyhow::anyhow!("failed")) } else { Ok(()) }
    });
    assert_eq!(result.err().map(|stopped| stopped.sent), Some(1));
    assert_eq!(ran, ["a", "b"]);
}

fn refused(sent: usize) -> Stopped {
    Stopped { sent, error: failure("paste-file", Some(4), b"error: this request is for another run\n") }
}

#[test]
fn a_hand_off_refused_for_its_run_at_its_first_command_says_nothing_was_sent() {
    assert_eq!(hand_refusal("vps", "vps.invalid", &refused(0)), "vps restarted: nothing was sent");
}

#[test]
fn a_hand_off_refused_for_its_run_after_a_command_went_says_part_was_sent() {
    assert_eq!(hand_refusal("vps", "vps.invalid", &refused(1)), "vps restarted after part was sent: hand it again");
}

#[test]
fn a_hand_off_failing_any_other_way_says_the_machine_and_the_reason_without_its_address() {
    let error = failure("send", Some(1), b"ssh: could not resolve hostname vps.invalid");
    let said = hand_refusal("vps", "vps.invalid", &Stopped { sent: 1, error });
    assert!(said.starts_with("vps: "), "{said}");
    assert!(!said.contains("vps.invalid"), "{said}");
}
