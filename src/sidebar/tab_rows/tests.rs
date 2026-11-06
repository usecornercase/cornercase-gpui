use super::*;
use crate::model::PaneInfo;

fn shell(id: u64) -> PaneInfo {
    PaneInfo { id, program: Some("zsh".into()), ..Default::default() }
}

fn opus(id: u64) -> PaneInfo {
    PaneInfo {
        id,
        agent: Some("claude".into()),
        model: Some("Opus 5.5".into()),
        context: Some(12),
        ..Default::default()
    }
}

fn details(tab: &Tab, mark: Option<&str>) -> String {
    tab_details(tab, mark, &Look::default())
}

#[test]
fn a_shell_tab_shows_its_name_and_program() {
    let tab = Tab { id: 1, name: "logs".into(), panes: vec![shell(2)], ..Default::default() };
    assert_eq!((tab_title(&tab), details(&tab, None)), ("logs".to_string(), "zsh".to_string()));
}

#[test]
fn a_shell_tab_named_after_its_program_does_not_repeat_it() {
    let tab = Tab { id: 1, name: "zsh".into(), panes: vec![shell(2), shell(3)], ..Default::default() };
    assert_eq!(details(&tab, None), "2 panes");
}

#[test]
fn a_shell_tab_shows_the_memory_it_uses() {
    let pane = PaneInfo { memory: Some(12 * 1024 * 1024), ..shell(2) };
    let tab = Tab { id: 1, name: "zsh".into(), panes: vec![pane], ..Default::default() };
    assert_eq!(details(&tab, None), "12 MB");
}

#[test]
fn an_agent_tab_shows_its_conversation_and_model() {
    let agent = PaneInfo { title: Some("✳ Fix the login".into()), ..opus(2) };
    let tab = Tab { id: 1, name: "claude".into(), panes: vec![agent, shell(3)], ..Default::default() };
    assert_eq!(tab_title(&tab), "Fix the login");
    assert_eq!(details(&tab, None), "claude · Opus 5.5 · 12% · 2 panes");
}

#[test]
fn an_agent_tab_names_its_account_after_the_model() {
    let tab = Tab { id: 1, name: "claude".into(), panes: vec![opus(2)], ..Default::default() };
    assert_eq!(details(&tab, Some("work")), "claude · Opus 5.5 · work · 12%");
}

#[test]
fn an_agent_tab_says_its_conversation_was_compacted() {
    let agent = PaneInfo { context: None, compacted: true, ..opus(2) };
    let tab = Tab { id: 1, name: "claude".into(), panes: vec![agent], ..Default::default() };
    assert_eq!(details(&tab, None), "claude · Opus 5.5 · compacted");
}

#[test]
fn an_agent_tab_adds_the_memory_of_its_panes_before_counting_them() {
    let agent = PaneInfo { memory: Some(1024 * 1024 * 1024), ..opus(2) };
    let beside = PaneInfo { memory: Some(288 * 1024 * 1024), ..shell(3) };
    let tab = Tab { id: 1, name: "claude".into(), panes: vec![agent, beside], ..Default::default() };
    assert_eq!(details(&tab, None), "claude · Opus 5.5 · 12% · 1.3 GB · 2 panes");
}

#[test]
fn the_model_and_context_follow_their_switches() {
    let tab = Tab { id: 1, name: "claude".into(), panes: vec![opus(2)], ..Default::default() };
    assert_eq!(tab_details(&tab, None, &Look { model: false, ..Look::default() }), "claude · 12%");
    assert_eq!(tab_details(&tab, None, &Look { context: false, ..Look::default() }), "claude · Opus 5.5");
}
