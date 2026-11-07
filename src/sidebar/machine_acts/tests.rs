use super::*;
use crate::model::Tab;
use crate::model::machines::seen::ThereId;

fn aim() -> Aim {
    Aim { machine: "vps".into(), boot: "b".into(), workspace: ThereId::heard(7) }
}

fn labels(entries: &[Entry]) -> Vec<(String, bool)> {
    entries
        .iter()
        .map(|entry| match entry {
            Entry::Item { label, .. } | Entry::Submenu { label, .. } => (label.to_string(), true),
            Entry::Resting { label } | Entry::Heading(label) | Entry::Note(label) => (label.to_string(), false),
            Entry::Separator => (String::new(), false),
        })
        .collect()
}

fn notes(tabs: usize, closed: Option<usize>) -> ThereWorkspace {
    ThereWorkspace {
        name: "notes".into(),
        tabs: vec![Tab::default(); tabs],
        closed_agents: closed,
        ..ThereWorkspace::default()
    }
}

#[test]
fn closing_names_the_workspace_and_the_machine() {
    let (title, _) = close_question(&notes(1, None), "vps");
    assert_eq!(title, "Close the workspace notes on vps?");
}

#[test]
fn closing_says_what_stops() {
    let (_, one) = close_question(&notes(1, None), "vps");
    assert_eq!(one, "Its tab and the programs running in it stop. Its folder stays on disk there.");
    let (_, two) = close_question(&notes(2, None), "vps");
    assert_eq!(two, "Its 2 tabs and the programs running in them stop. Its folder stays on disk there.");
}

#[test]
fn closing_adds_the_closed_agents_the_snapshot_counts() {
    let (_, one) = close_question(&notes(0, Some(1)), "vps");
    assert_eq!(one, "Its folder stays on disk there. Its closed agent is forgotten.");
    let (_, two) = close_question(&notes(0, Some(3)), "vps");
    assert_eq!(two, "Its folder stays on disk there. Its 3 closed agents are forgotten.");
}

#[test]
fn a_linked_machines_menu_offers_only_the_rename() {
    let entries = there_entries(Ok(()), aim());
    assert_eq!(labels(&entries), [(RENAME_WORKSPACE.to_string(), true)]);
}

#[test]
fn a_machine_that_is_not_linked_says_why_and_rests_the_rename() {
    let entries = there_entries(Err("vps unreachable".to_string()), aim());
    assert_eq!(labels(&entries), [("vps unreachable".to_string(), false), (RENAME_WORKSPACE.to_string(), false)]);
}

fn pane_aim(collapse: bool) -> PaneAim {
    PaneAim { machine: "vps".into(), boot: "b".into(), pane: ThereId::heard(9), collapse }
}

#[test]
fn a_machines_agent_offers_only_its_fold() {
    assert_eq!(labels(&there_agent_entries(Ok(()), pane_aim(true))), [("Collapse".to_string(), true)]);
    assert_eq!(labels(&there_agent_entries(Ok(()), pane_aim(false))), [("Expand".to_string(), true)]);
}

#[test]
fn a_machine_out_of_reach_says_why_and_greys_the_fold() {
    let entries = there_agent_entries(Err("vps unreachable".into()), pane_aim(true));
    assert_eq!(labels(&entries), [("vps unreachable".to_string(), false), ("Collapse".to_string(), false)]);
}
