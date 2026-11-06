use super::*;
use crate::model::PaneInfo;

fn agent(model: Option<&str>, context: Option<u32>) -> PaneInfo {
    PaneInfo { agent: Some("claude".into()), model: model.map(str::to_string), context, ..Default::default() }
}

fn tab_of(panes: Vec<PaneInfo>) -> Tab {
    Tab { panes, ..Default::default() }
}

fn detail(pane: PaneInfo, mark: Option<&str>) -> Option<String> {
    tab_detail(&tab_of(vec![pane]), mark, &Look::default())
}

#[test]
fn a_tab_shows_its_model_and_context() {
    assert_eq!(detail(agent(Some("Opus 5.5"), Some(12)), None).as_deref(), Some("Opus 5.5 · 12%"));
}

#[test]
fn a_tab_names_its_account_after_the_model() {
    assert_eq!(detail(agent(Some("Opus 5.5"), Some(12)), Some("work")).as_deref(), Some("Opus 5.5 · work · 12%"));
}

#[test]
fn a_tab_without_a_model_still_names_its_account() {
    assert_eq!(detail(agent(None, None), Some("custom")).as_deref(), Some("custom"));
}

#[test]
fn a_tab_without_a_model_or_an_account_shows_nothing() {
    assert_eq!(detail(agent(None, Some(12)), None), None);
}

#[test]
fn a_compacted_conversation_reads_after_the_model() {
    let pane = PaneInfo { compacted: true, ..agent(Some("Opus 5.5"), None) };
    assert_eq!(detail(pane, None).as_deref(), Some("Opus 5.5 · compacted"));
}

#[test]
fn a_tab_ends_with_the_memory_of_its_panes() {
    let shell = PaneInfo { memory: Some(1024 * 1024 * 1024), ..Default::default() };
    let pane = PaneInfo { memory: Some(288 * 1024 * 1024), ..agent(Some("Opus 5.5"), Some(23)) };
    let tab = tab_of(vec![pane, shell]);
    assert_eq!(tab_detail(&tab, None, &Look::default()).as_deref(), Some("Opus 5.5 · 23% · 1.3 GB"));
}

#[test]
fn a_shell_tab_shows_its_memory_alone() {
    let tab = tab_of(vec![PaneInfo { memory: Some(12 * 1024 * 1024), ..Default::default() }]);
    assert_eq!(tab_detail(&tab, None, &Look::default()).as_deref(), Some("12 MB"));
}

#[test]
fn the_model_and_context_switches_leave_the_memory_alone() {
    let tab = tab_of(vec![PaneInfo { memory: Some(12 * 1024 * 1024), ..agent(Some("Opus 5.5"), Some(23)) }]);
    let off = Look { model: false, context: false, ..Look::default() };
    assert_eq!(tab_detail(&tab, None, &off).as_deref(), Some("12 MB"));
}

#[test]
fn with_the_model_off_the_context_still_shows() {
    let tab = tab_of(vec![agent(Some("Opus 5.5"), Some(23))]);
    let off = Look { model: false, ..Look::default() };
    assert_eq!(tab_detail(&tab, None, &off).as_deref(), Some("23%"));
}
