use super::{Go, go, left_its_pane, out_of_reach};
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
    assert_eq!(
        out_of_reach("api › login › claude", &place("c1")),
        "api › login › claude cannot be reached: it is closed"
    );
}

#[test]
fn a_pull_request_out_of_reach_says_it_has_no_address() {
    assert_eq!(out_of_reach("#212", &place("p1")), "#212 cannot be reached: it has no web address");
}

#[test]
fn leaving_its_pane_names_the_issue_it_waits_on() {
    let note = left_its_pane("shop › main › checkout", "workspace");
    assert!(note.starts_with("shop › main › checkout is not in its pane any more: showing its workspace."), "{note}");
    assert!(note.ends_with("needs usecornercase/cornercase-terminal#664"), "{note}");
}
