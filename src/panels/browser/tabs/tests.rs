use serde_json::json;

use super::*;

fn event(session: Option<&str>, method: &str, params: Value) -> Event {
    Event { session: session.map(str::to_string), method: method.to_string(), params }
}

fn tabs_with(target: &str, session: &str, workspace: Option<u64>) -> Tabs {
    let mut tabs = Tabs::default();
    let mut tab = Tab::new(target, "about:blank", workspace);
    tab.session = Some(session.to_string());
    tabs.add(tab);
    tabs
}

#[test]
fn a_workspace_shows_its_own_tabs_and_the_unclaimed_ones() {
    let mut tabs = Tabs::default();
    tabs.add(Tab::new("A", "http://a", Some(1)));
    tabs.add(Tab::new("B", "http://b", Some(2)));
    tabs.add(Tab::new("C", "http://c", None));
    let shown: Vec<&str> = tabs.shown(Some(1)).map(|t| t.target.as_str()).collect();
    assert_eq!(shown, ["A", "C"]);
}

#[test]
fn each_workspace_remembers_its_active_tab() {
    let mut tabs = Tabs::default();
    tabs.add(Tab::new("A", "http://a", Some(1)));
    tabs.add(Tab::new("A2", "http://a2", Some(1)));
    tabs.add(Tab::new("B", "http://b", Some(2)));
    tabs.activate(Some(1), "A");
    assert_eq!(tabs.active_target(Some(1)).as_deref(), Some("A"));
    assert_eq!(tabs.active_target(Some(2)).as_deref(), Some("B"));
}

#[test]
fn closing_the_active_tab_falls_back_to_the_last_one() {
    let mut tabs = Tabs::default();
    tabs.add(Tab::new("A", "http://a", Some(1)));
    tabs.add(Tab::new("A2", "http://a2", Some(1)));
    tabs.activate(Some(1), "A2");
    let closed = tabs.apply(&event(None, "Target.targetDestroyed", json!({ "targetId": "A2" })));
    assert_eq!(closed, Followup::Closed("A2".into()));
    assert_eq!(tabs.active_target(Some(1)).as_deref(), Some("A"));
}

#[test]
fn a_main_frame_navigation_moves_the_address() {
    let mut tabs = tabs_with("T", "S", Some(1));
    let params = json!({ "frame": { "id": "F", "url": "http://localhost:5173/login" } });
    assert_eq!(tabs.apply(&event(Some("S"), "Page.frameNavigated", params)), Followup::Navigated("T".into()));
    assert_eq!(tabs.get("T").map(|t| t.url.as_str()), Some("http://localhost:5173/login"));
}

#[test]
fn an_iframe_navigation_leaves_the_address() {
    let mut tabs = tabs_with("T", "S", Some(1));
    let params = json!({ "frame": { "id": "I", "parentId": "F", "url": "http://ads" } });
    assert_eq!(tabs.apply(&event(Some("S"), "Page.frameNavigated", params)), Followup::Nothing);
    assert_eq!(tabs.get("T").map(|t| t.url.as_str()), Some("about:blank"));
}

#[test]
fn a_history_push_in_the_main_frame_moves_the_address() {
    let mut tabs = tabs_with("T", "S", Some(1));
    tabs.apply(&event(Some("S"), "Page.frameNavigated", json!({ "frame": { "id": "F", "url": "http://x/" } })));
    let params = json!({ "frameId": "F", "url": "http://x/#/settings" });
    tabs.apply(&event(Some("S"), "Page.navigatedWithinDocument", params));
    assert_eq!(tabs.get("T").map(|t| t.url.as_str()), Some("http://x/#/settings"));
}

#[test]
fn the_title_follows_the_target_info() {
    let mut tabs = tabs_with("T", "S", None);
    let params = json!({ "targetInfo": { "targetId": "T", "title": "Shop", "url": "http://shop" } });
    tabs.apply(&event(None, "Target.targetInfoChanged", params));
    assert_eq!(tabs.get("T").map(Tab::label), Some("Shop"));
}

#[test]
fn loading_ends_with_the_load_event() {
    let mut tabs = tabs_with("T", "S", None);
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
