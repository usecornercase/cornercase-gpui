use serde_json::json;

use super::model::{Catalogue, State, Step, state_of};
use crate::panels::PanelKind;
use crate::test_util::fixture;

fn catalogue() -> Catalogue {
    serde_json::from_str(fixture!("tour/list.json")).expect("the tours parse")
}

fn step(done: serde_json::Value, show: Option<serde_json::Value>, target: &str) -> Step {
    Step { done, show, target: target.into(), ..Step::default() }
}

#[test]
fn the_active_step_is_found_in_its_tour() {
    let c = catalogue();
    let (tour, index, step) = c.current().expect("a tour is active");
    assert_eq!((tour.id.as_str(), index, step.id.as_str()), ("todo", 1, "add"));
}

#[test]
fn no_tour_is_current_without_an_active_one() {
    let mut c = catalogue();
    c.state.active = None;
    assert!(c.current().is_none());
}

#[test]
fn progress_counts_the_steps_done_and_says_when_a_tour_is_finished() {
    let c = catalogue();
    let label = |id: &str| c.progress_label(c.tour(id).expect("listed"));
    assert_eq!(label("core"), "2/7");
    assert_eq!(label("issues"), "done");
    assert_eq!(label("changes"), "0/3");
}

#[test]
fn a_step_shows_the_panel_it_talks_about() {
    let panel = step(json!({"kind": "next"}), Some(json!({"kind": "panel", "panel": "changes"})), "changes");
    assert_eq!(panel.shows(), Some(PanelKind::Changes));
    let settings = step(json!({"kind": "next"}), Some(json!({"kind": "settings", "page": "agents"})), "settings");
    assert_eq!(settings.shows(), Some(PanelKind::Settings));
    let menu = step(json!({"kind": "next"}), Some(json!({"kind": "help-menu"})), "help");
    assert_eq!(menu.shows(), None);
}

#[test]
fn opening_a_panel_or_a_sheet_completes_the_step_that_asks_for_it() {
    assert_eq!(step(json!({"kind": "panel", "panel": "todo"}), None, "todo").completed_by(), Some(PanelKind::Todo));
    let issues = step(json!({"kind": "opened", "dialog": "issues"}), None, "issues");
    assert_eq!(issues.completed_by(), Some(PanelKind::Issues));
    assert_eq!(step(json!({"kind": "tab-opened"}), None, "tabs").completed_by(), None);
}

#[test]
fn only_an_explanatory_step_moves_on_with_next() {
    assert!(step(json!({"kind": "next"}), None, "none").explains());
    assert!(!step(json!({"kind": "project-opened"}), None, "new-project").explains());
}

#[test]
fn a_step_points_at_the_rail_icon_of_its_panel() {
    assert_eq!(step(json!({}), None, "search").points_at(), Some(PanelKind::Search));
    assert_eq!(step(json!({}), None, "tabs").points_at(), None);
}

#[test]
fn the_state_comes_from_the_app_feed() {
    let app = json!({"tour": {"active": {"tour": "core", "step": "tabs", "index": 2, "card": false}, "progress": []}});
    let state = state_of(Some(&app));
    assert_eq!(state.active.map(|a| a.step), Some("tabs".to_string()));
    assert_eq!(state_of(None), State::default());
}
