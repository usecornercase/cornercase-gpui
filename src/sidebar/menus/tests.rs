use super::context_item;
use crate::model::PaneInfo;

#[test]
fn a_pane_running_an_agent_offers_its_context() {
    let pane = PaneInfo { id: 1, agent: Some("claude".into()), ..PaneInfo::default() };
    assert!(context_item(&pane).is_some());
}

#[test]
fn a_pane_whose_agent_left_offers_no_context() {
    assert!(context_item(&PaneInfo::default()).is_none());
}
