use super::*;
use crate::cli::Remote;
use crate::model::{ThereId, ThereProject, ThereWorkspace};

fn workspace(id: u64, name: &str) -> ThereWorkspace {
    ThereWorkspace {
        id: ThereId::heard(id),
        name: name.into(),
        path: format!("/home/ana/code/{name}"),
        ..ThereWorkspace::default()
    }
}

fn seen(state: LinkState) -> MachineSeen {
    let projects = vec![ThereProject {
        name: "shop".into(),
        workspaces: vec![workspace(7, "main"), workspace(9, "notes")],
        ..ThereProject::default()
    }];
    MachineSeen { name: "vps".into(), state, boot: "b1".into(), projects, ..MachineSeen::default() }
}

fn listed() -> Vec<Machine> {
    vec![Machine { name: "vps".into(), destination: "ana@vps.example.com".into(), ..Machine::default() }]
}

fn read(state: LinkState, window: &Target) -> MachineRead {
    read_seen(&seen(state), window)
}

fn read_seen(seen: &MachineSeen, window: &Target) -> MachineRead {
    let ws = seen.workspace(ThereId::heard(7)).expect("a workspace");
    read_of(&MachinePlace::at(seen, ws, None), seen, window, &listed(), Some("/home/ana"))
}

const TRIES_AT: &str = "1970-01-01T00:16:44.139Z";
const NOW: i64 = 1_000;

fn other_server() -> Target {
    Target::Remote(Remote::new("devbox", "").expect("a destination"))
}

#[test]
fn a_linked_machine_is_read_at_its_own_destination() {
    let read = read(LinkState::Linked, &Target::Local);
    let target = read.reach.target().expect("reached");
    assert_eq!(target.flags(), ["--remote", "ana@vps.example.com"]);
}

#[test]
fn a_machine_down_has_nothing_to_ask_with_and_comes_back_by_itself() {
    let read = read(LinkState::Unreachable, &Target::Local);
    assert_eq!(read.reach, Reach::Down("vps unreachable".into()));
    assert!(read.reach.target().is_none());
}

#[test]
fn a_machine_connecting_counts_as_coming_back() {
    assert!(matches!(read(LinkState::Connecting, &Target::Local).reach, Reach::Down(_)));
}

#[test]
fn an_older_machine_waits_for_someone_rather_than_coming_back() {
    let read = read(LinkState::Older, &Target::Local);
    assert_eq!(read.reach, Reach::Refused("vps runs an older cornercase".into()));
}

#[test]
fn a_window_on_another_server_never_reads_a_machine_even_when_it_is_linked() {
    let read = read(LinkState::Linked, &other_server());
    assert!(read.reach.target().is_none());
    assert!(matches!(read.reach, Reach::Refused(_)));
}

#[test]
fn a_window_on_another_server_does_not_promise_to_reconnect_to_a_machine_that_is_down() {
    assert!(matches!(read(LinkState::Unreachable, &other_server()).reach, Reach::Refused(_)));
}

#[test]
fn a_read_names_the_place_and_the_folder_as_that_machine_writes_it() {
    let read = read(LinkState::Linked, &Target::Local);
    assert_eq!(read.place, "shop › main");
    assert_eq!(read.path, "/home/ana/code/main");
    assert_eq!(read.folder, "~/code/main");
    assert_eq!(read.aim.workspace_arg(), "7");
}

#[test]
fn without_the_machines_home_the_folder_shows_whole() {
    let seen = seen(LinkState::Linked);
    let ws = seen.workspace(ThereId::heard(7)).expect("a workspace");
    let read = read_of(&MachinePlace::at(&seen, ws, None), &seen, &Target::Local, &listed(), None);
    assert_eq!(read.folder, "/home/ana/code/main");
}

fn place_then(later: &MachineSeen) -> MachineRead {
    let picked = seen(LinkState::Linked);
    let ws = picked.workspace(ThereId::heard(7)).expect("a workspace");
    read_of(&MachinePlace::at(&picked, ws, None), later, &Target::Local, &listed(), None)
}

#[test]
fn a_place_the_snapshot_no_longer_lists_keeps_its_own_name() {
    let emptied = MachineSeen { projects: Vec::new(), ..seen(LinkState::Connecting) };
    assert_eq!(place_then(&emptied).place, "main");
}

#[test]
fn the_numbers_of_another_run_do_not_name_a_place_kept_while_the_link_is_down() {
    let projects = vec![ThereProject {
        name: "other".into(),
        workspaces: vec![workspace(7, "another")],
        ..ThereProject::default()
    }];
    let restarted = MachineSeen { boot: "b2".into(), projects, ..seen(LinkState::Unreachable) };
    assert_eq!(place_then(&restarted).place, "main");
}

#[test]
fn a_read_names_the_destination_it_goes_through_also_while_the_machine_is_down() {
    assert_eq!(read(LinkState::Linked, &Target::Local).destination, "ana@vps.example.com");
    assert_eq!(read(LinkState::Unreachable, &Target::Local).destination, "ana@vps.example.com");
}

#[test]
fn a_machine_gone_from_the_list_has_no_destination() {
    let seen = seen(LinkState::Linked);
    let ws = seen.workspace(ThereId::heard(7)).expect("a workspace");
    let read = read_of(&MachinePlace::at(&seen, ws, None), &seen, &Target::Local, &[], None);
    assert_eq!(read.destination, "");
}

#[test]
fn a_read_of_a_link_that_fell_says_when_it_tries_next() {
    let fell = MachineSeen { retry_at: Some(TRIES_AT.into()), ..seen(LinkState::Unreachable) };
    assert_eq!(read_seen(&fell, &Target::Local).next_try(NOW), Some(NextTry::In(4)));
}

#[test]
fn a_read_of_a_linked_machine_promises_no_try() {
    assert_eq!(read(LinkState::Linked, &Target::Local).next_try(NOW), None);
}

fn tab(down: Option<&str>) -> TabThere {
    TabThere {
        machine: "vps".into(),
        boot: "b1".into(),
        target: Target::Local,
        down: down.map(str::to_string),
        agents: vec![5],
    }
}

fn reach_of(tab: Option<&TabThere>, machine: &str, boot: &str, pane: u64) -> AgentReach {
    reach_agent(tab, machine, boot, pane)
}

#[test]
fn an_agent_on_a_live_tab_can_be_read() {
    assert_eq!(reach_of(Some(&tab(None)), "vps", "b1", 5), AgentReach::Ready(Target::Local));
}

#[test]
fn a_pane_that_runs_no_agent_or_is_gone_reads_as_gone() {
    let gone = AgentReach::Gone("that agent is no longer on vps".into());
    assert_eq!(reach_of(Some(&tab(None)), "vps", "b1", 6), gone);
    assert_eq!(reach_of(Some(&tab(None)), "vps", "b1", 99), gone);
}

#[test]
fn a_restart_or_another_tab_reads_as_gone() {
    let live = tab(None);
    assert_eq!(reach_of(Some(&live), "vps", "b0", 5), AgentReach::Gone("vps restarted".into()));
    let none = AgentReach::Gone("The tab on vps is no longer shown".into());
    assert_eq!(reach_of(None, "vps", "b1", 5), none);
    assert_eq!(reach_of(Some(&live), "mini", "b1", 5), AgentReach::Gone("The tab on mini is no longer shown".into()));
}

#[test]
fn a_tab_that_is_down_reads_as_down_even_when_the_agent_is_listed() {
    let down = tab(Some("vps unreachable"));
    assert_eq!(reach_of(Some(&down), "vps", "b1", 5), AgentReach::Down("vps unreachable".into()));
}
