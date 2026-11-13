use super::PanelKind;
use crate::model::Session;

fn session(shown: u64) -> Session {
    let text = format!(
        r#"{{"projects":[{{"id":1,"name":"shop","workspaces":[
        {{"id":2,"name":"main","tabs":[{{"id":3,"panes":[{{"id":4,"agent":"claude","role":null}}]}}]}},
        {{"id":5,"name":"fix-login","tabs":[{{"id":6,"panes":[{{"id":7,"agent":"claude","role":"orchestrator"}}]}},
            {{"id":8,"panes":[{{"id":9,"agent":"codex","role":"coder"}}]}}]}}]}}],
        "view":{{"id":1,"shown":{{"project":1,"workspace":{shown},"tab":null,"pane":null}}}}}}"#
    );
    serde_json::from_str(&text).expect("a session")
}

#[test]
fn the_flow_panel_shows_only_while_the_shown_workspace_runs_a_flow() {
    assert!(PanelKind::Flow.shows(Some(&session(5))));
    assert!(!PanelKind::Flow.shows(Some(&session(2))));
    assert!(!PanelKind::Flow.shows(None));
}

#[test]
fn the_other_docked_panels_show_in_any_workspace() {
    for kind in [PanelKind::Changes, PanelKind::Files, PanelKind::Todo, PanelKind::Recap, PanelKind::Browser] {
        assert!(kind.shows(Some(&session(2))), "{kind:?}");
        assert!(kind.shows(None), "{kind:?}");
    }
}

#[test]
fn the_flow_panel_is_docked_beside_the_terminal() {
    assert!(PanelKind::Flow.docked());
    assert!(!PanelKind::Flows.docked());
}

#[test]
fn every_cached_panel_is_docked() {
    assert!(PanelKind::ALL.into_iter().filter(|kind| kind.cached()).all(PanelKind::docked));
}

#[test]
fn only_the_four_panels_that_read_a_machine_ask_it_again_when_shown() {
    let reading: Vec<PanelKind> = PanelKind::ALL.into_iter().filter(|kind| kind.reads_machine()).collect();
    assert_eq!(reading, [PanelKind::Changes, PanelKind::Files, PanelKind::Browser, PanelKind::Ports]);
}
