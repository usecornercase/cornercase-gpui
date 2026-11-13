use super::*;
use crate::model::Owner;
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
    let done = finished(&working_agents(before, &[]), &working_agents(after, &[]), after);
    done.into_iter()
        .filter_map(|(working, owner)| match (working, owner) {
            (Working::Here(pane), Owner::Here(workspace)) => Some((workspace, pane)),
            _ => None,
        })
        .collect()
}

#[test]
fn only_working_agents_are_watched() {
    assert_eq!(working_agents(&session(), &[]), HashMap::from([(Working::Here(1000), Owner::Here(10))]));
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
    let mut add = |target: &str, url: &str, owner: Owner| {
        let mut tab = Tab::new(target, url, owner);
        tab.session = Some(format!("s-{target}"));
        tabs.add(tab);
    };
    add("A", "http://localhost:5173/", Owner::Here(10));
    add("B", "https://example.com/", Owner::Here(10));
    add("C", "about:blank", Owner::Here(10));
    add("D", "http://localhost:3000/", Owner::Here(11));
    add("E", "http://localhost:4000/", Owner::Nobody);
    assert_eq!(reloadable(&tabs, &Owner::Here(10)), ["s-A", "s-B"]);
}

fn machine_session(status: &str) -> Session {
    let mut session: Session = serde_json::from_str(fixture!("machines/view.json")).expect("a view line");
    for seen in &mut session.machines {
        seen.state = crate::model::machines::LinkState::Linked;
        let panes = seen.projects.iter_mut().flat_map(|p| &mut p.workspaces).flat_map(|w| &mut w.tabs);
        for pane in panes.flat_map(|t| &mut t.panes).filter(|p| p.agent.is_some()) {
            pane.status = Some(status.to_string());
        }
    }
    session
}

#[test]
fn a_machine_agent_finishing_reloads_only_its_workspaces_tabs() {
    let (before, after) = (machine_session("working"), machine_session("done"));
    let now = working_agents(&after, &[]);
    let was = working_agents(&before, &[]);
    assert!(!was.is_empty() && now.is_empty());
    let done = finished(&was, &now, &after);
    assert_eq!(done.len(), was.len());
    let (working, owner) = done.first().expect("one finished");
    assert!(matches!(working, Working::There(_)) && owner.machine() == Some("vps"));
    let mut tabs = Tabs::default();
    let mut tab = Tab::new("M", "http://localhost:5173/", owner.clone());
    tab.session = Some("s-M".into());
    tabs.add(tab);
    let mut ours = Tab::new("H", "http://localhost:5173/", Owner::Here(1));
    ours.session = Some("s-H".into());
    tabs.add(ours);
    assert_eq!(reloadable(&tabs, owner), ["s-M"]);
}

#[test]
fn a_machine_restart_is_no_finish() {
    let before = machine_session("working");
    let mut after = machine_session("working");
    after.machines[0].boot = "another".into();
    let was = working_agents(&before, &[]);
    let now: HashMap<Working, Owner> = HashMap::new();
    assert_eq!(finished(&was, &now, &after), []);
}
