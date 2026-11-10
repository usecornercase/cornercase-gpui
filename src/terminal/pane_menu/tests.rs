use super::*;
use crate::context_menu::read_entries;
use crate::model::{PaneInfo, Project, Tab, Workspace};

fn pane(id: u64, agent: Option<&str>, program: &str) -> PaneInfo {
    PaneInfo { id, agent: agent.map(str::to_string), program: Some(program.into()), ..PaneInfo::default() }
}

fn session(panes: Vec<PaneInfo>) -> Session {
    let tab = Tab { id: 1, panes, ..Tab::default() };
    let workspace = Workspace { id: 1, tabs: vec![tab], ..Workspace::default() };
    Session {
        projects: vec![Project { id: 1, workspaces: vec![workspace], ..Project::default() }],
        ..Session::default()
    }
}

fn three() -> Session {
    session(vec![pane(5, None, "zsh"), pane(6, Some("claude"), "node"), pane(7, None, "zsh")])
}

fn move_here(this: &mut Root, pane: u64, aim: Aim, cx: &mut Context<Root>) {
    this.move_pane(pane, aim, cx);
}

#[test]
fn a_dropped_pane_asks_cornercase_to_move_it_beside_its_target() {
    let args = move_pane_args(4, Aim { target: 7, place: Place::Below });
    assert_eq!(args, ["move-pane", "--pane", "4", "--target", "7", "--place", "below", "--json"]);
}

#[test]
fn a_move_on_a_machine_asks_for_the_layout_and_names_its_run() {
    let args = crate::model::with_boot(move_pane_args(4, Aim { target: 7, place: Place::Below }), "b1");
    assert_eq!(args, ["move-pane", "--pane", "4", "--target", "7", "--place", "below", "--json", "--boot", "b1"]);
}

#[test]
fn the_targets_are_the_other_panes_of_its_tab_named_as_the_menu_names_them() {
    let targets = move_targets(Some(&three()), 5);
    assert_eq!(targets, [(6, "claude (pane 6)".to_string()), (7, "zsh (pane 7)".to_string())]);
}

#[test]
fn a_pane_alone_has_nowhere_to_move() {
    let alone = session(vec![pane(5, None, "zsh")]);
    assert_eq!(move_targets(Some(&alone), 5), Vec::<(u64, String)>::new());
    assert!(move_menu(5, Vec::new(), &move_here).is_none());
    assert_eq!(move_targets(None, 5), Vec::<(u64, String)>::new());
}

#[test]
fn move_pane_offers_each_target_with_the_five_places() {
    let menu = move_menu(5, move_targets(Some(&three()), 5), &move_here).expect("a menu");
    let Entry::Submenu { label, entries } = menu else { panic!("expected a submenu") };
    assert_eq!(label.to_string(), "Move pane");
    let named = [("submenu", "claude (pane 6)".to_string()), ("submenu", "zsh (pane 7)".to_string())];
    assert_eq!(read_entries(&entries), named);
    let Entry::Submenu { entries: places, .. } = &entries[0] else { panic!("expected places") };
    let labels: Vec<String> = read_entries(places).into_iter().map(|(_, label)| label).collect();
    assert_eq!(labels, ["Swap with it", "Left of it", "Right of it", "Above it", "Below it"]);
}
