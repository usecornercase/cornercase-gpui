use super::*;
use crate::model::machines::LinkState;
use crate::model::{PaneInfo, ThereProject, ThereWorkspace};
use crate::store::OlderNote;

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

const NOW: i64 = 1_000;

fn told(home: Option<&str>) -> Told<'_> {
    Told { home, error: None, now: NOW }
}

fn text_of(seen: &MachineSeen, home: Option<&str>) -> CardText {
    let ws = seen.workspace(ThereId::heard(1)).expect("the workspace");
    card_text(&MachinePlace::at(seen, ws, None), seen, &told(home), &Standing::Connecting)
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

fn standing_on(state: LinkState, standing: &Standing) -> CardText {
    let seen = machine(state, vec![agent(3, Some("working"), true)]);
    let ws = seen.workspace(ThereId::heard(1)).expect("the workspace");
    card_text(&MachinePlace::at(&seen, ws, None), &seen, &told(None), standing)
}

const TRIES_AT: &str = "1970-01-01T00:16:44.139Z";

fn fallen(retry_at: Option<&str>) -> MachineSeen {
    let seen = machine(LinkState::Unreachable, vec![agent(3, Some("working"), true)]);
    MachineSeen { retry_at: retry_at.map(str::to_string), ..seen }
}

fn fallen_says(retry_at: Option<&str>, told: &Told<'_>) -> CardText {
    let seen = fallen(retry_at);
    let ws = seen.workspace(ThereId::heard(1)).expect("the workspace");
    card_text(&MachinePlace::at(&seen, ws, None), &seen, told, &Standing::LinkDown)
}

#[test]
fn the_card_says_it_connects_until_the_first_frame() {
    let text = standing_on(LinkState::Linked, &Standing::Connecting);
    assert_eq!(text.state, Some(CardState::Running("connecting…".to_string())));
}

#[test]
fn the_card_over_a_link_that_fell_counts_down_to_its_next_try() {
    let text = fallen_says(Some(TRIES_AT), &told(None));
    assert_eq!(text.title, "vps unreachable");
    assert_eq!(text.line, "claude keeps working there");
    assert_eq!(text.state, Some(CardState::Said("reconnecting in 4 s".to_string())));
}

#[test]
fn the_card_says_the_try_is_under_way_once_its_time_has_passed() {
    let later = Told { now: NOW + 4, ..told(None) };
    assert_eq!(fallen_says(Some(TRIES_AT), &later).state, Some(CardState::Running("reconnecting…".to_string())));
    let long_after = Told { now: NOW + 60, ..told(None) };
    assert_eq!(fallen_says(Some(TRIES_AT), &long_after).state, Some(CardState::Running("reconnecting…".to_string())));
}

#[test]
fn the_card_over_a_link_that_waits_on_a_person_says_why_and_promises_no_try() {
    let error = "cornercase was not found on vps";
    let text = fallen_says(None, &Told { error: Some(error), ..told(None) });
    assert_eq!(text.state, Some(CardState::Said(error.to_string())));
    assert_eq!(fallen_says(None, &told(None)).state, None);
}

#[test]
fn the_card_over_a_link_that_is_connecting_says_it_reconnects() {
    let text = standing_on(LinkState::Connecting, &Standing::LinkDown);
    assert_eq!(text.state, Some(CardState::Running("reconnecting…".to_string())));
}

#[test]
fn the_card_over_a_try_of_its_own_says_it_reconnects() {
    let text = standing_on(LinkState::Linked, &Standing::Reconnecting);
    assert_eq!(text.state, Some(CardState::Running("reconnecting…".to_string())));
}

#[test]
fn the_card_asks_for_a_tick_only_while_it_shows_a_time() {
    let counting = fallen(Some(TRIES_AT));
    assert!(card_counts(&Standing::LinkDown, &counting, NOW));
    assert!(!card_counts(&Standing::LinkDown, &counting, NOW + 4));
    assert!(!card_counts(&Standing::LinkDown, &fallen(None), NOW));
    assert!(card_counts(&Standing::RetryIn(4, None), &fallen(None), NOW));
    assert!(!card_counts(&Standing::Shown, &counting, NOW));
}

#[test]
fn the_card_over_a_stream_that_fell_counts_down() {
    let text = standing_on(LinkState::Linked, &Standing::RetryIn(4, Some("ssh: refused".to_string())));
    assert_eq!(text.title, "runs on vps");
    assert_eq!(text.state, Some(CardState::Said("reconnecting in 4 s".to_string())));
    assert_eq!(text.why.as_deref(), Some("ssh: refused"));
}

#[test]
fn the_card_says_when_no_tab_is_open_there() {
    let text = standing_on(LinkState::Linked, &Standing::NoTab);
    assert_eq!(text.line, "no tab is open there");
    assert_eq!(text.state, None);
}

#[test]
fn the_card_of_a_window_on_another_server_says_what_it_needs() {
    let text = standing_on(LinkState::Linked, &Standing::Elsewhere("why".to_string()));
    assert_eq!(text.line, format!("its tab needs this window on this {}", desktop::here()));
    assert_eq!(text.state, Some(CardState::Said("why".to_string())));
}

#[test]
fn the_card_of_an_older_machine_waits_on_a_person_and_promises_nothing() {
    let text = standing_on(LinkState::Older, &Standing::LinkDown);
    assert_eq!(text.title, "vps runs an older cornercase");
    assert_eq!(text.state, None);
}

fn card_in(standing: &Standing) -> CardText {
    let seen = machine(LinkState::Linked, Vec::new());
    let ws = seen.workspace(ThereId::heard(1)).expect("the workspace");
    card_text(&MachinePlace::at(&seen, ws, None), &seen, &told(None), standing)
}

#[test]
fn the_card_waits_for_the_machines_list_with_a_spinner() {
    let text = card_in(&Standing::Waiting("waiting for the machines list".into()));
    assert_eq!(text.state, Some(CardState::Running("waiting for the machines list…".into())));
    assert_eq!(text.why, None);
}

#[test]
fn the_card_of_a_machine_refused_as_older_says_both_releases_and_how_to_update() {
    let note =
        OlderNote { line: "vps runs cornercase 0.13.39 and this Mac 0.13.57: update it".into(), tip: "do it".into() };
    let text = card_in(&Standing::Older(note.clone()));
    assert_eq!(text.state, Some(CardState::Said(note.line)));
    assert_eq!(text.why.as_deref(), Some("do it"));
}
