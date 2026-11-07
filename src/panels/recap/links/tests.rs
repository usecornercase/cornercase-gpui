use super::{Go, go, out_of_reach};
use crate::model::Session;
use crate::panels::recap::data::{Answer, Named, Place};
use crate::test_util::fixture;

const BOOT: &str = "2640531-1793928893554110246";

fn session() -> Session {
    let text = format!(
        r#"{{"boot":"{BOOT}","projects":[{{"id":5,"name":"shop","path":"/home/ana/code/shop","workspaces":[
        {{"id":6,"name":"main","path":"/home/ana/code/shop","tabs":[{{"id":8,"panes":[{{"id":9,"agent":"claude"}}]}}]}}]}}]}}"#
    );
    serde_json::from_str(&text).expect("a session")
}

fn place(id: &str) -> Place {
    let answer: Answer = serde_json::from_str(fixture!("recap/now.json")).expect("an answer");
    answer.places.get(id).cloned().expect("a place")
}

fn moved() -> Place {
    let answer: Answer = serde_json::from_str(fixture!("recap/moved.json")).expect("an answer");
    answer.places.get("c1").cloned().expect("a place")
}

fn reopened() -> Session {
    serde_json::from_str(fixture!("recap/reopened-status.json")).expect("a session")
}

const REOPENED_PANE: u64 = 12;
const SHELL_PANE: u64 = 7;

#[test]
fn a_conversation_reopened_in_another_pane_goes_to_that_pane() {
    assert_eq!(go(&moved(), Some(&reopened())), Go::Pane(REOPENED_PANE));
}

#[test]
fn a_conversation_that_came_back_after_a_restart_goes_to_its_pane() {
    let earlier = Place { boot: Some("1-earlier".to_string()), ..moved() };
    assert_eq!(go(&earlier, Some(&reopened())), Go::Pane(REOPENED_PANE));
}

#[test]
fn the_conversation_counts_before_the_pane_the_place_names() {
    let boot = reopened().boot;
    let named = Place { pane: Some(SHELL_PANE), boot: Some(boot), ..moved() };
    assert_eq!(go(&named, Some(&reopened())), Go::Pane(REOPENED_PANE));
}

#[test]
fn another_agent_s_conversation_of_that_id_is_not_it() {
    let other = Place { agent: Some("codex".to_string()), ..moved() };
    assert_eq!(go(&other, Some(&reopened())), Go::Workspace(6));
}

#[test]
fn a_pane_that_reports_no_agent_is_named_by_its_conversation_alone() {
    let mut session = reopened();
    let panes = session.projects.iter_mut().flat_map(|p| &mut p.workspaces).flat_map(|w| &mut w.tabs);
    panes.flat_map(|t| &mut t.panes).for_each(|pane| pane.agent = None);
    assert_eq!(go(&moved(), Some(&session)), Go::Pane(REOPENED_PANE));
}

#[test]
fn a_place_without_a_conversation_still_tries_its_pane() {
    let boot = reopened().boot;
    let opencode = Place { conversation: None, pane: Some(SHELL_PANE), boot: Some(boot), ..moved() };
    assert_eq!(go(&opencode, Some(&reopened())), Go::Pane(SHELL_PANE));
}

#[test]
fn a_place_without_a_conversation_is_not_in_a_pane_without_one() {
    let opencode = Place { conversation: None, ..moved() };
    assert_eq!(go(&opencode, Some(&reopened())), Go::Workspace(6));
}

#[test]
fn a_conversation_in_a_live_pane_of_this_server_goes_to_it() {
    assert_eq!(go(&place("c3"), Some(&session())), Go::Pane(9));
}

#[test]
fn a_pane_closed_since_goes_to_its_workspace() {
    let closed = Place { pane: Some(4242), ..place("c3") };
    assert_eq!(go(&closed, Some(&session())), Go::Workspace(6));
}

#[test]
fn a_pane_of_an_earlier_server_is_not_this_server_s_pane_of_that_number() {
    let earlier = Place { boot: Some("1-earlier".to_string()), ..place("c3") };
    assert_eq!(go(&earlier, Some(&session())), Go::Workspace(6));
}

#[test]
fn a_place_without_a_boot_is_not_trusted_for_its_pane() {
    let no_boot = Place { boot: None, ..place("c3") };
    assert_eq!(go(&no_boot, Some(&session())), Go::Workspace(6));
}

#[test]
fn a_workspace_closed_since_goes_to_its_project() {
    let gone = Named { label: "gone".to_string(), path: "/home/ana/code/shop-gone".to_string() };
    let closed = Place { pane: None, workspace: Some(gone), ..place("c3") };
    assert_eq!(go(&closed, Some(&session())), Go::Project(5));
}

#[test]
fn a_project_closed_since_cannot_be_reached() {
    assert_eq!(go(&place("c1"), Some(&session())), Go::Nowhere);
}

#[test]
fn a_conversation_with_no_place_cannot_be_reached() {
    assert_eq!(go(&Place::default(), Some(&session())), Go::Nowhere);
}

#[test]
fn before_the_session_a_conversation_cannot_be_reached() {
    assert_eq!(go(&place("c3"), None), Go::Nowhere);
}

#[test]
fn a_pull_request_opens_its_address() {
    let address = "https://github.com/acme/shop/pull/212".to_string();
    assert_eq!(go(&place("p1"), Some(&session())), Go::Address(address.clone()));
    assert_eq!(go(&place("p1"), None), Go::Address(address));
}

#[test]
fn a_pull_request_without_a_web_address_cannot_be_reached() {
    assert_eq!(go(&Place { url: None, ..place("p1") }, None), Go::Nowhere);
    assert_eq!(go(&Place { url: Some("file:///etc/passwd".to_string()), ..place("p1") }, None), Go::Nowhere);
}

#[test]
fn a_conversation_out_of_reach_says_it_is_closed() {
    assert_eq!(out_of_reach("api › login › claude", &place("c1")), "api › login › claude is closed");
}

#[test]
fn a_pull_request_out_of_reach_says_it_has_no_address() {
    assert_eq!(out_of_reach("#212", &place("p1")), "#212 cannot be reached: it has no web address");
}
