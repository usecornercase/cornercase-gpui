use super::*;
use crate::model::machines::LinkState;
use crate::model::{PaneInfo, ThereProject, ThereWorkspace};

fn agent(id: u64, status: Option<&str>, active: bool) -> PaneInfo<ThereId> {
    PaneInfo {
        id: ThereId::heard(id),
        agent: Some("claude".into()),
        status: status.map(str::to_string),
        active,
        ..PaneInfo::default()
    }
}

fn machine(state: LinkState, panes: Vec<PaneInfo<ThereId>>) -> MachineSeen {
    let tab = Tab { id: ThereId::heard(2), active: true, panes, ..Tab::default() };
    let ws = ThereWorkspace {
        id: ThereId::heard(1),
        name: "notes".into(),
        path: "/home/ana/code/shop".into(),
        tabs: vec![tab],
        ..ThereWorkspace::default()
    };
    let project = ThereProject { workspaces: vec![ws], ..ThereProject::default() };
    MachineSeen { name: "vps".into(), state, projects: vec![project], ..MachineSeen::default() }
}

fn text_of(seen: &MachineSeen, home: Option<&str>) -> CardText {
    let ws = seen.workspace(ThereId::heard(1)).expect("the workspace");
    card_text(&MachinePlace::at(seen, ws, None), seen, home)
}

#[test]
fn the_card_says_where_it_runs_and_what_its_agent_does() {
    let seen = machine(LinkState::Linked, vec![agent(3, Some("waiting"), true)]);
    let text = text_of(&seen, None);
    assert_eq!((text.title.as_str(), text.line.as_str()), ("runs on vps", "claude is waiting for you"));
}

#[test]
fn the_card_words_each_status() {
    let line = |status| text_of(&machine(LinkState::Linked, vec![agent(3, status, true)]), None).line;
    assert_eq!(line(Some("working")), "claude is working");
    assert_eq!(line(Some("done")), "claude is done");
    assert_eq!(line(Some("failed")), "claude failed");
    assert_eq!(line(None), "claude is idle");
}

#[test]
fn the_card_reads_the_active_panes_agent_first() {
    let waiting = agent(3, Some("waiting"), false);
    let mut working = agent(4, Some("working"), true);
    working.agent = Some("codex".into());
    let text = text_of(&machine(LinkState::Linked, vec![waiting, working]), None);
    assert_eq!(text.line, "codex is working");
}

#[test]
fn the_card_says_when_there_is_no_agent() {
    let shell = PaneInfo { program: Some("zsh".into()), ..PaneInfo::default() };
    assert_eq!(text_of(&machine(LinkState::Linked, vec![shell]), None).line, "no agent there");
}

#[test]
fn the_card_shows_the_folder_with_the_machines_home_as_a_tilde() {
    let seen = machine(LinkState::Linked, Vec::new());
    assert_eq!(text_of(&seen, Some("/home/ana")).folder, "~/code/shop");
    assert_eq!(text_of(&seen, None).folder, "/home/ana/code/shop");
}

#[test]
fn while_down_the_card_says_the_agent_keeps_working_there() {
    let text = text_of(&machine(LinkState::Unreachable, vec![agent(3, Some("working"), true)]), None);
    assert_eq!((text.title.as_str(), text.line.as_str()), ("vps unreachable", "claude keeps working there"));
}

#[test]
fn while_down_the_card_of_a_shell_says_its_shells_keep_running() {
    let text = text_of(&machine(LinkState::Unreachable, Vec::new()), None);
    assert_eq!(text.line, "its shells keep running there");
}
