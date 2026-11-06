use super::*;
use crate::model::machines::LinkState;
use crate::model::{PaneInfo, Workspace};

const SEEN: &str = "2026-11-06T01:23:55.000Z";

fn now() -> i64 {
    crate::panels::flows::model::epoch(SEEN).expect("a time") + 120
}

fn machine(state: LinkState) -> MachineSeen {
    MachineSeen { name: "vps".into(), state, seen: Some(SEEN.into()), ..MachineSeen::default() }
}

fn agent_tab() -> Tab<ThereId> {
    let pane = PaneInfo {
        agent: Some("claude".into()),
        status: Some("waiting".into()),
        model: Some("Opus 5.5".into()),
        context: Some(40),
        ..PaneInfo::default()
    };
    Tab { name: "claude".into(), panes: vec![pane], ..Tab::default() }
}

#[test]
fn a_fold_lives_here_and_can_be_undone() {
    let (seen, ws) = (machine(LinkState::Linked), ThereWorkspace::default());
    let mut folds = ThereFolds::default();
    let fold = FoldThere::workspace(&seen, &ws);
    assert!(!folds.folded(&fold));
    folds.toggle(fold.clone());
    assert!(folds.folded(&fold));
    folds.toggle(fold.clone());
    assert!(!folds.folded(&fold));
}

#[test]
fn two_workspaces_in_one_folder_fold_apart() {
    let seen = machine(LinkState::Linked);
    let main = ThereWorkspace { name: "main".into(), path: "/home/ana/code/shop".into(), ..ThereWorkspace::default() };
    let notes = ThereWorkspace { name: "notes".into(), ..main.clone() };
    let mut folds = ThereFolds::default();
    folds.toggle(FoldThere::workspace(&seen, &main));
    assert!(!folds.folded(&FoldThere::workspace(&seen, &notes)));
}

#[test]
fn a_fold_survives_the_machines_restart() {
    let seen = machine(LinkState::Linked);
    let before = ThereWorkspace { name: "notes".into(), path: "/p".into(), ..ThereWorkspace::default() };
    let mut folds = ThereFolds::default();
    folds.toggle(FoldThere::workspace(&seen, &before));
    let restarted = MachineSeen { boot: "later".into(), ..seen };
    assert!(folds.folded(&FoldThere::workspace(&restarted, &before)));
}

#[test]
fn a_project_folds_apart_from_its_workspace_in_the_same_folder() {
    let seen = machine(LinkState::Linked);
    let there = ThereProject { path: "/p".into(), ..ThereProject::default() };
    let ws = ThereWorkspace { name: "main".into(), path: "/p".into(), ..ThereWorkspace::default() };
    let mut folds = ThereFolds::default();
    folds.toggle(FoldThere::project(&seen, &there));
    assert!(!folds.folded(&FoldThere::workspace(&seen, &ws)));
}

#[test]
fn a_row_says_its_branch_when_it_differs_from_its_name() {
    let ws = ThereWorkspace { name: "notes".into(), branch: Some("main".into()), ..ThereWorkspace::default() };
    assert_eq!(there_detail(&ws, false), ["main"]);
    let same = ThereWorkspace { branch: Some("notes".into()), ..ws };
    assert_eq!(there_detail(&same, false), Vec::<String>::new());
}

#[test]
fn a_folded_row_counts_its_tabs_and_agents() {
    let ws = ThereWorkspace { tabs: vec![Tab::default(), agent_tab()], ..ThereWorkspace::default() };
    assert_eq!(there_detail(&ws, true), ["2 tabs", "1 agent"]);
}

#[test]
fn a_row_says_how_many_agents_were_closed() {
    let ws = ThereWorkspace { closed_agents: Some(2), ..ThereWorkspace::default() };
    assert_eq!(there_detail(&ws, false), ["2 closed"]);
}

#[test]
fn a_tab_on_a_linked_machine_is_detailed_as_ours() {
    let details = there_tab_details(&agent_tab(), "claude", &Look::default());
    assert_eq!(details.as_deref(), Some("claude · Opus 5.5 · 40%"));
    assert_eq!(machine(LinkState::Linked).tab_note(&agent_tab(), now()), None);
}

#[test]
fn a_tab_on_a_machine_says_it_was_compacted_and_what_it_uses() {
    let mut tab = agent_tab();
    tab.panes[0].context = None;
    tab.panes[0].compacted = true;
    tab.panes[0].memory = Some(12 * 1024 * 1024);
    let details = there_tab_details(&tab, "claude", &Look::default());
    assert_eq!(details.as_deref(), Some("claude · Opus 5.5 · compacted · 12 MB"));
}

#[test]
fn a_tab_on_a_machine_follows_the_model_and_context_switches() {
    let off = Look { model: false, context: false, ..Look::default() };
    assert_eq!(there_tab_details(&agent_tab(), "there", &off).as_deref(), Some("claude"));
}

#[test]
fn a_tab_with_an_agent_on_a_machine_that_is_down_says_when_it_was_seen() {
    let note = machine(LinkState::Unreachable).tab_note(&agent_tab(), now());
    assert_eq!(note.as_deref(), Some("waiting · seen 2 min ago"));
}

#[test]
fn details_that_repeat_the_title_are_left_out() {
    let pane = PaneInfo { program: Some("zsh".into()), ..PaneInfo::default() };
    let tab = Tab { name: "zsh".into(), panes: vec![pane], ..Tab::default() };
    assert_eq!(there_tab_details(&tab, "zsh", &Look::default()), None);
}

#[test]
fn a_local_workspaces_row_ids_never_match_a_machines() {
    let seen = machine(LinkState::Linked);
    let local = Workspace { id: 7, ..Workspace::default() };
    assert_ne!(there_id("ws", &seen, ThereId::heard(7)), SharedString::from(format!("ws-{}", local.id)));
}
