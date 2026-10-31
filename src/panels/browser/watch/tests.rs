use super::*;
use crate::panels::browser::tabs::Tab;
use crate::test_util::fixture;

fn session() -> Session {
    serde_json::from_str(fixture!("browser/session.json")).expect("the session parses")
}

fn with_status(pane: u64, status: Option<&str>) -> Session {
    let mut session = session();
    let panes = session.projects.iter_mut().flat_map(|p| &mut p.workspaces).flat_map(|w| &mut w.tabs);
    let found = panes.flat_map(|t| &mut t.panes).find(|p| p.id == pane).expect("the pane exists");
    found.status = status.map(str::to_string);
    session
}

fn finishes(before: &Session, after: &Session) -> Vec<(u64, u64)> {
    finished(&working_agents(before), &working_agents(after), after)
}

#[test]
fn only_working_agents_are_watched() {
    assert_eq!(working_agents(&session()), HashMap::from([(1000, 10)]));
}

#[test]
fn an_agent_that_stops_working_finishes_in_its_workspace() {
    assert_eq!(finishes(&session(), &with_status(1000, Some("done"))), [(10, 1000)]);
}

#[test]
fn an_agent_that_waits_for_an_answer_has_finished_its_turn() {
    assert_eq!(finishes(&session(), &with_status(1000, Some("waiting"))), [(10, 1000)]);
}

#[test]
fn an_agent_still_working_has_not_finished() {
    assert_eq!(finishes(&session(), &session()), [] as [(u64, u64); 0]);
}

#[test]
fn an_agent_that_starts_working_has_not_finished() {
    assert_eq!(finishes(&with_status(1000, Some("idle")), &session()), [] as [(u64, u64); 0]);
}

#[test]
fn a_closed_agent_has_not_finished() {
    let mut closed = session();
    closed.projects[0].workspaces[0].tabs.remove(0);
    assert_eq!(finishes(&session(), &closed), [] as [(u64, u64); 0]);
}

#[test]
fn a_finish_is_seen_once() {
    let done = with_status(1000, Some("done"));
    assert_eq!(finishes(&done, &done), [] as [(u64, u64); 0]);
}

#[test]
fn only_the_workspaces_web_pages_reload() {
    let mut tabs = Tabs::default();
    let mut add = |target: &str, url: &str, workspace: Option<u64>| {
        let mut tab = Tab::new(target, url, workspace);
        tab.session = Some(format!("s-{target}"));
        tabs.add(tab);
    };
    add("A", "http://localhost:5173/", Some(10));
    add("B", "https://example.com/", Some(10));
    add("C", "about:blank", Some(10));
    add("D", "http://localhost:3000/", Some(11));
    add("E", "http://localhost:4000/", None);
    assert_eq!(reloadable(&tabs, 10), ["s-A", "s-B"]);
}
