use serde_json::json;

use super::*;

fn event(session: Option<&str>, method: &str, params: Value) -> Event {
    Event { session: session.map(str::to_string), method: method.to_string(), params }
}

fn tabs_with(target: &str, session: &str, owner: Owner) -> Tabs {
    let mut tabs = Tabs::default();
    let mut tab = Tab::new(target, "about:blank", owner);
    tab.session = Some(session.to_string());
    tabs.add(tab);
    tabs
}

#[test]
fn a_workspace_shows_its_own_tabs_and_the_unclaimed_ones() {
    let mut tabs = Tabs::default();
    tabs.add(Tab::new("A", "http://a", Owner::Here(1)));
    tabs.add(Tab::new("B", "http://b", Owner::Here(2)));
    tabs.add(Tab::new("C", "http://c", Owner::Nobody));
    let shown: Vec<&str> = tabs.shown(&Owner::Here(1)).map(|t| t.target.as_str()).collect();
    assert_eq!(shown, ["A", "C"]);
}

#[test]
fn each_workspace_remembers_its_active_tab() {
    let mut tabs = Tabs::default();
    tabs.add(Tab::new("A", "http://a", Owner::Here(1)));
    tabs.add(Tab::new("A2", "http://a2", Owner::Here(1)));
    tabs.add(Tab::new("B", "http://b", Owner::Here(2)));
    tabs.activate(Owner::Here(1), "A");
    assert_eq!(tabs.active_target(&Owner::Here(1)).as_deref(), Some("A"));
    assert_eq!(tabs.active_target(&Owner::Here(2)).as_deref(), Some("B"));
}

#[test]
fn closing_the_active_tab_falls_back_to_the_last_one() {
    let mut tabs = Tabs::default();
    tabs.add(Tab::new("A", "http://a", Owner::Here(1)));
    tabs.add(Tab::new("A2", "http://a2", Owner::Here(1)));
    tabs.activate(Owner::Here(1), "A2");
    let closed = tabs.apply(&event(None, "Target.targetDestroyed", json!({ "targetId": "A2" })));
    assert_eq!(closed, Followup::Closed("A2".into()));
    assert_eq!(tabs.active_target(&Owner::Here(1)).as_deref(), Some("A"));
}

#[test]
fn a_main_frame_navigation_moves_the_address() {
    let mut tabs = tabs_with("T", "S", Owner::Here(1));
    let params = json!({ "frame": { "id": "F", "url": "http://localhost:5173/login" } });
    assert_eq!(tabs.apply(&event(Some("S"), "Page.frameNavigated", params)), Followup::Navigated("T".into()));
    assert_eq!(tabs.get("T").map(|t| t.url.as_str()), Some("http://localhost:5173/login"));
}

#[test]
fn an_iframe_navigation_leaves_the_address() {
    let mut tabs = tabs_with("T", "S", Owner::Here(1));
    let params = json!({ "frame": { "id": "I", "parentId": "F", "url": "http://ads" } });
    assert_eq!(tabs.apply(&event(Some("S"), "Page.frameNavigated", params)), Followup::Nothing);
    assert_eq!(tabs.get("T").map(|t| t.url.as_str()), Some("about:blank"));
}

#[test]
fn a_history_push_in_the_main_frame_moves_the_address() {
    let mut tabs = tabs_with("T", "S", Owner::Here(1));
    tabs.apply(&event(Some("S"), "Page.frameNavigated", json!({ "frame": { "id": "F", "url": "http://x/" } })));
    let params = json!({ "frameId": "F", "url": "http://x/#/settings" });
    tabs.apply(&event(Some("S"), "Page.navigatedWithinDocument", params));
    assert_eq!(tabs.get("T").map(|t| t.url.as_str()), Some("http://x/#/settings"));
}

#[test]
fn the_title_follows_the_target_info() {
    let mut tabs = tabs_with("T", "S", Owner::Nobody);
    let params = json!({ "targetInfo": { "targetId": "T", "title": "Shop", "url": "http://shop" } });
    tabs.apply(&event(None, "Target.targetInfoChanged", params));
    assert_eq!(tabs.get("T").map(Tab::label), Some("Shop"));
}

#[test]
fn loading_ends_with_the_load_event() {
    let mut tabs = tabs_with("T", "S", Owner::Nobody);
    tabs.apply(&event(Some("S"), "Page.loadEventFired", json!({})));
    assert_eq!(tabs.get("T").map(|t| t.loading), Some(false));
}

#[test]
fn history_steps_name_the_entries_around_the_current_one() {
    let history = json!({ "currentIndex": 1, "entries": [{ "id": 10 }, { "id": 11 }, { "id": 12 }] });
    assert_eq!(history_steps(&history), (Some(10), Some(12)));
    let first = json!({ "currentIndex": 0, "entries": [{ "id": 10 }] });
    assert_eq!(history_steps(&first), (None, None));
}

fn machine(id: u64) -> Owner {
    use crate::model::machines::seen::{MachineSeen, ThereId, ThereProject, ThereWorkspace, WorkspaceThere};
    let ws = ThereWorkspace { id: ThereId::heard(id), ..Default::default() };
    let project = ThereProject { workspaces: vec![ws.clone()], ..Default::default() };
    let seen = MachineSeen { name: "vps".into(), projects: vec![project], ..Default::default() };
    Owner::There(WorkspaceThere::at(&seen, &ws, "vps.invalid"))
}

#[test]
fn a_machine_tab_never_shows_under_our_workspace_with_the_same_id() {
    let mut tabs = Tabs::default();
    tabs.add(Tab::new("here", "http://a", Owner::Here(7)));
    tabs.add(Tab::new("there", "http://b", machine(7)));
    let ours: Vec<&str> = tabs.shown(&Owner::Here(7)).map(|t| t.target.as_str()).collect();
    let theirs: Vec<&str> = tabs.shown(&machine(7)).map(|t| t.target.as_str()).collect();
    assert_eq!((ours, theirs), (vec!["here"], vec!["there"]));
}

#[test]
fn each_place_keeps_its_active_tab_across_a_visit_elsewhere() {
    let mut tabs = Tabs::default();
    for (target, owner) in [("a1", Owner::Here(1)), ("a2", Owner::Here(1)), ("m1", machine(5)), ("m2", machine(5))] {
        tabs.add(Tab::new(target, "http://x", owner));
    }
    tabs.activate(Owner::Here(1), "a1");
    tabs.activate(machine(5), "m1");
    for place in [Owner::Here(1), machine(5), Owner::Here(1), machine(5)] {
        let expected = if place == Owner::Here(1) { "a1" } else { "m1" };
        assert_eq!(tabs.active_target(&place).as_deref(), Some(expected));
    }
}

#[test]
fn rekeying_an_owner_moves_its_tabs_and_active_tab() {
    let mut tabs = Tabs::default();
    tabs.add(Tab::new("m1", "http://x", machine(5)));
    tabs.add(Tab::new("m2", "http://y", machine(5)));
    tabs.activate(machine(5), "m1");
    tabs.rekey(&machine(5), &machine(9));
    assert!(tabs.shown(&machine(5)).next().is_none());
    assert_eq!(tabs.active_target(&machine(9)).as_deref(), Some("m1"));
}

#[test]
fn a_place_shows_its_tabs_and_agents_tabs_only() {
    let mut tabs = Tabs::default();
    tabs.add(Tab::new("a", "http://a", Owner::Here(1)));
    tabs.add(Tab::new("m", "http://m", machine(1)));
    tabs.add(Tab::new("n", "http://n", Owner::Nobody));
    let shown: Vec<&str> = tabs.shown(&machine(1)).map(|t| t.target.as_str()).collect();
    assert_eq!(shown, ["m", "n"]);
}
