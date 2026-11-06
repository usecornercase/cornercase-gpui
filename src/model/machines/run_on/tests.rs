use std::time::{Duration, Instant};

use super::clone::Cloned;
use super::*;
use crate::model::machines::{LinkState, parse_list};
use crate::model::{Session, ThereId};
use crate::test_util::fixture;

const NOW: i64 = 1_793_933_900;

fn session() -> Session {
    serde_json::from_str(fixture!("machines/view.json")).expect("a view line")
}

fn list() -> Vec<Machine> {
    parse_list(fixture!("machines/list.json")).expect("the list").machines
}

fn offer<'a>(offers: &'a [Offer], name: &str) -> &'a Offer {
    offers.iter().find(|offer| offer.name == name).expect("an offer of that name")
}

fn project(session: &Session, id: u64) -> &Project {
    session.projects.iter().find(|project| project.id == id).expect("a project here")
}

fn looking<'a>(session: &'a Session, list: &'a [Machine], project_id: u64) -> Looking<'a> {
    Looking {
        machine: "vps",
        project: Some(project(session, project_id)),
        list,
        seen: &session.machines,
        cloning: false,
        cloned: &[],
        now: Instant::now(),
    }
}

fn ready(there: There) -> Spot {
    match there {
        There::Ready(spot) => spot,
        other => panic!("not ready: {other:?}"),
    }
}

fn spot_of_shop(session: &Session) -> Spot {
    let list = list();
    ready(looking(session, &list, 8).there())
}

fn new_project(session: &mut Session, id: u64, repo: &str) {
    let ours = Project { id, name: "more".into(), repo: Some(repo.into()), ..Project::default() };
    session.projects.push(ours);
}

fn cloned(session: &Session, project: u64, there: u64) -> Cloned {
    let seen = session.machine("vps").expect("vps");
    Cloned { machine: "vps".into(), boot: seen.boot.clone(), project, there: ThereId::heard(there), at: Instant::now() }
}

#[test]
fn a_linked_machine_with_a_snapshot_can_be_picked() {
    let offers = offers(&list(), &session().machines, None, NOW);
    assert_eq!(offer(&offers, "vps").why, None);
}

#[test]
fn a_machine_down_says_why_and_what_ssh_said() {
    let offers = offers(&list(), &session().machines, None, NOW);
    let homelab = offer(&offers, "homelab");
    assert_eq!(homelab.why.as_deref(), Some("unreachable · not seen yet"));
    assert_eq!(homelab.said.as_deref(), Some("ssh: connect to host homelab.example.com port 22: Connection refused"));
}

#[test]
fn a_machine_linked_with_no_snapshot_yet_cannot_be_picked() {
    let offers = offers(&list(), &[], None, NOW);
    assert_eq!(offer(&offers, "vps").why.as_deref(), Some("not seen yet"));
}

#[test]
fn from_another_server_no_machine_can_be_picked() {
    let offers = offers(&list(), &session().machines, Some("devbox"), NOW);
    let vps = offer(&offers, "vps");
    assert_eq!(vps.why.as_deref(), Some("linked from devbox"));
    assert_eq!(vps.said.as_deref(), Some(FROM_ANOTHER_SERVER));
}

#[test]
fn the_project_the_machine_pairs_with_is_ready_with_its_own_id() {
    let spot = spot_of_shop(&session());
    assert_eq!(spot.project, ThereId::heard(8));
    assert_eq!(spot.path, "/home/ana/code/shop");
}

#[test]
fn another_clone_here_of_the_same_repository_finds_the_project_there_too() {
    let mut session = session();
    new_project(&mut session, 40, "GitHub.com/Acme/Shop");
    let list = list();
    assert_eq!(ready(looking(&session, &list, 40).there()).project, ThereId::heard(8));
}

#[test]
fn a_project_without_a_remote_cannot_run_there() {
    let (session, list) = (session(), list());
    assert_eq!(looking(&session, &list, 4).there(), There::NoRemote);
}

#[test]
fn a_project_that_is_not_there_asks_for_a_clone() {
    let mut session = session();
    new_project(&mut session, 40, "github.com/acme/only-here");
    let list = list();
    assert_eq!(looking(&session, &list, 40).there(), There::NotCloned);
}

#[test]
fn a_clone_on_its_way_holds_the_start() {
    let (session, list) = (session(), list());
    let cloning = Looking { cloning: true, ..looking(&session, &list, 8) };
    assert_eq!(cloning.there(), There::Cloning);
}

#[test]
fn a_clone_that_answered_waits_until_the_machine_lists_it() {
    let mut session = session();
    new_project(&mut session, 40, "github.com/acme/only-here");
    let (list, cloned) = (list(), [cloned(&session, 40, 30)]);
    let answered = Looking { cloned: &cloned, ..looking(&session, &list, 40) };
    assert_eq!(answered.there(), There::Listing);
}

#[test]
fn a_clone_the_machine_never_lists_asks_for_another_clone_after_a_while() {
    let mut session = session();
    new_project(&mut session, 40, "github.com/acme/only-here");
    let (list, cloned) = (list(), [cloned(&session, 40, 30)]);
    let later = Instant::now() + Duration::from_secs(31);
    let answered = Looking { cloned: &cloned, now: later, ..looking(&session, &list, 40) };
    assert_eq!(answered.there(), There::NotCloned);
}

#[test]
fn a_clone_the_machine_lists_is_ready_under_another_repository() {
    let mut session = session();
    new_project(&mut session, 40, "github.com/acme/only-here");
    let (list, cloned) = (list(), [cloned(&session, 40, 20)]);
    let answered = Looking { cloned: &cloned, ..looking(&session, &list, 40) };
    assert_eq!(ready(answered.there()).project, ThereId::heard(20));
}

#[test]
fn a_clone_from_before_the_machine_restarted_counts_for_nothing() {
    let mut session = session();
    new_project(&mut session, 40, "github.com/acme/only-here");
    let mut old = cloned(&session, 40, 20);
    old.boot = "older".into();
    let (list, cloned) = (list(), [old]);
    let answered = Looking { cloned: &cloned, ..looking(&session, &list, 40) };
    assert_eq!(answered.there(), There::NotCloned);
}

#[test]
fn a_machine_that_went_down_stays_chosen_and_refuses() {
    let mut session = session();
    session.machines[0].state = LinkState::Unreachable;
    let list = list();
    assert_eq!(looking(&session, &list, 8).there(), There::Refused("vps unreachable".into()));
}

#[test]
fn a_machine_with_no_snapshot_refuses_by_name() {
    let (session, list) = (session(), list());
    let seen: &[MachineSeen] = &[];
    let looking = Looking { seen, ..looking(&session, &list, 8) };
    assert_eq!(looking.there(), There::Refused("vps is not linked".into()));
}

#[test]
fn a_machine_that_left_the_list_says_so() {
    let session = session();
    let looking = Looking { list: &[], ..looking(&session, &[], 8) };
    assert_eq!(looking.there(), There::Refused("vps is no longer listed".into()));
}

#[test]
fn no_place_chosen_yet_refuses() {
    let (session, list) = (session(), list());
    let looking = Looking { project: None, ..looking(&session, &list, 8) };
    assert_eq!(looking.there(), There::Refused("Choose a project first".into()));
}

fn answer(text: &str) -> Answer {
    parse_answer(text).expect("an answer")
}

#[test]
fn an_agent_started_there_is_awaited_by_its_tab() {
    assert_eq!(answer(fixture!("machines/there_started.json")).item(), Some(Item::Tab(ThereId::heard(33))));
}

#[test]
fn a_workspace_made_there_is_awaited_by_its_tab() {
    assert_eq!(answer(fixture!("machines/there_workspace.json")).item(), Some(Item::Tab(ThereId::heard(12))));
}

#[test]
fn a_flow_started_there_is_awaited_by_its_orchestrators_tab() {
    assert_eq!(answer(fixture!("machines/there_flow.json")).item(), Some(Item::Tab(ThereId::heard(22))));
}

#[test]
fn a_login_tab_is_awaited_by_its_tab() {
    assert_eq!(answer(fixture!("machines/there_new_tab.json")).item(), Some(Item::Tab(ThereId::heard(26))));
}

#[test]
fn an_answer_with_no_id_is_a_failure() {
    assert_eq!(parse_answer("{}"), Err(ANSWER_UNREAD.to_string()));
    assert_eq!(parse_answer("not json"), Err(ANSWER_UNREAD.to_string()));
}

#[test]
fn only_an_agent_can_hold_its_answer_while_it_asks_something() {
    let agent = Starting::Agent { agent: "claude".into(), issue: "#7".into() };
    assert!(agent.waits_for_its_agent());
    assert!(!Starting::Flow { name: "Quick".into() }.waits_for_its_agent());
    assert!(!Starting::Workspace { name: "notes".into() }.waits_for_its_agent());
}

#[test]
fn a_flow_started_there_says_where_to_watch_it() {
    let done = Starting::Flow { name: "Quick".into() }.done("vps", &answer(fixture!("machines/there_flow.json")));
    assert_eq!(
        done,
        "Started Quick on vps, on probe-flow. Follow it in its tab: the run page reads this computer's runs"
    );
}

#[test]
fn the_job_names_what_starts_and_where() {
    let agent = Starting::Agent { agent: "claude".into(), issue: "#7".into() };
    assert_eq!(agent.doing("vps"), "Starting claude on #7 on vps…");
    assert_eq!(Starting::Workspace { name: "notes".into() }.doing("vps"), "Creating notes on vps…");
}

fn at() -> Instant {
    Instant::now()
}

fn awaited_answer(spot: &Spot, file: &str, now: Instant) -> Awaited {
    Awaited::answered(spot, &answer(file), now).expect("an id to wait for")
}

#[test]
fn what_started_is_not_selected_before_the_machine_lists_it() {
    let session = session();
    let awaited = awaited_answer(&spot_of_shop(&session), fixture!("machines/there_started.json"), at());
    assert_eq!(awaited.look(&session.machines, at()), Look::NotYet);
}

#[test]
fn a_tab_the_machine_lists_is_selected_in_its_workspace() {
    let session = session();
    let mut awaited = awaited_answer(&spot_of_shop(&session), fixture!("machines/there_started.json"), at());
    awaited.item = Item::Tab(ThereId::heard(16));
    let Look::Listed(place) = awaited.look(&session.machines, at()) else { panic!("listed") };
    assert_eq!((place.workspace, place.tab), (ThereId::heard(11), Some(ThereId::heard(16))));
}

#[test]
fn a_workspace_the_machine_lists_is_selected_on_its_active_tab() {
    let session = session();
    let mut awaited = awaited_answer(&spot_of_shop(&session), fixture!("machines/there_started.json"), at());
    awaited.item = Item::Workspace(ThereId::heard(11));
    let Look::Listed(place) = awaited.look(&session.machines, at()) else { panic!("listed") };
    assert_eq!((place.workspace, place.tab), (ThereId::heard(11), Some(ThereId::heard(10))));
}

#[test]
fn a_new_tab_in_the_project_is_selected_while_the_agent_holds_its_answer() {
    let mut session = session();
    let mut spot = spot_of_shop(&session);
    spot.tabs.remove(&ThereId::heard(14));
    let awaited = Awaited::new_tab_in(&spot, Some(1), at());
    let Look::Listed(place) = awaited.look(&session.machines, at()) else { panic!("listed") };
    assert_eq!(place.tab, Some(ThereId::heard(14)));
    session.machines[0].projects[1].workspaces.clear();
    assert_eq!(awaited.look(&session.machines, at()), Look::NotYet);
}

#[test]
fn no_new_tab_means_nothing_to_select_yet() {
    let session = session();
    let awaited = Awaited::new_tab_in(&spot_of_shop(&session), Some(1), at());
    assert_eq!(awaited.look(&session.machines, at()), Look::NotYet);
}

#[test]
fn a_machine_that_restarted_drops_what_was_awaited() {
    let mut session = session();
    let awaited = awaited_answer(&spot_of_shop(&session), fixture!("machines/there_started.json"), at());
    session.machines[0].boot = "another".into();
    assert_eq!(awaited.look(&session.machines, at()), Look::Gone);
}

#[test]
fn a_machine_that_is_not_seen_for_now_keeps_it_awaited() {
    let session = session();
    let awaited = awaited_answer(&spot_of_shop(&session), fixture!("machines/there_started.json"), at());
    assert_eq!(awaited.look(&[], at()), Look::NotYet);
}

#[test]
fn what_never_shows_is_given_up_after_a_while() {
    let session = session();
    let now = at();
    let awaited = awaited_answer(&spot_of_shop(&session), fixture!("machines/there_started.json"), now);
    assert_eq!(awaited.look(&session.machines, now + Duration::from_secs(31)), Look::Gone);
}

#[test]
fn only_the_early_wait_of_that_project_takes_the_answer() {
    let session = session();
    let spot = spot_of_shop(&session);
    assert!(Awaited::new_tab_in(&spot, Some(1), at()).takes_answer_of(&spot));
    assert!(!awaited_answer(&spot, fixture!("machines/there_started.json"), at()).takes_answer_of(&spot));
    let other = Spot { project: ThereId::heard(4), ..spot.clone() };
    assert!(!Awaited::new_tab_in(&spot, Some(1), at()).takes_answer_of(&other));
}
