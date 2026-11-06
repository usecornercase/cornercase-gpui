use super::*;
use crate::model::PaneInfo;

fn shell(id: u64) -> PaneInfo {
    PaneInfo { id, program: Some("zsh".into()), ..Default::default() }
}

#[test]
fn a_shell_tab_shows_its_name_and_program() {
    let tab = Tab { id: 1, name: "logs".into(), panes: vec![shell(2)], ..Default::default() };
    assert_eq!((tab_title(&tab), tab_details(&tab, None)), ("logs".to_string(), "zsh".to_string()));
}

#[test]
fn an_agent_tab_shows_its_conversation_and_model() {
    let agent = PaneInfo {
        id: 2,
        agent: Some("claude".into()),
        title: Some("✳ Fix the login".into()),
        model: Some("Opus 5.5".into()),
        context: Some(12),
        ..Default::default()
    };
    let tab = Tab { id: 1, name: "claude".into(), panes: vec![agent, shell(3)], ..Default::default() };
    assert_eq!(tab_title(&tab), "Fix the login");
    assert_eq!(tab_details(&tab, None), "claude · Opus 5.5 · 12% · 2 panes");
}

#[test]
fn an_agent_tab_names_its_account_after_the_model() {
    let agent = PaneInfo {
        id: 2,
        agent: Some("claude".into()),
        model: Some("Opus 5.5".into()),
        context: Some(12),
        ..Default::default()
    };
    let tab = Tab { id: 1, name: "claude".into(), panes: vec![agent], ..Default::default() };
    assert_eq!(tab_details(&tab, Some("work")), "claude · Opus 5.5 · work · 12%");
}
