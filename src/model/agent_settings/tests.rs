use serde_json::json;

use super::*;
use crate::test_util::fixture;

fn there() -> Agents {
    serde_json::from_str(fixture!("machines/there_config.json")).expect("what the machine answered")
}

fn claude() -> Modes {
    there().modes_of("claude").expect("claude's modes").clone()
}

fn codex() -> Modes {
    there().modes_of("codex").expect("codex's modes").clone()
}

#[test]
fn this_computers_config_gives_the_default_agent_the_list_and_each_agents_modes() {
    let config = json!({"settings": [
        {"key": "agent", "value": "auto", "choices": [
            {"value": "auto", "note": "the agent in your tab"}, {"value": "claude", "note": "claude"},
            {"value": "codex", "note": "codex"}]},
        {"key": "agents.claude.mode", "value": "plan", "choices": [
            {"value": "default", "dangerous": false}, {"value": "plan"},
            {"value": "skip permissions (dangerous)", "dangerous": true}]},
        {"key": "agents.claude.extra", "value": ""}
    ]});
    let agents = Agents::from_config(&config);
    assert_eq!(agents.default, "auto");
    assert_eq!(agents.list.iter().map(|a| a.value.as_str()).collect::<Vec<_>>(), ["claude", "codex"]);
    assert_eq!(agents.initial(), "claude");
    let modes = &agents.modes["claude"];
    assert_eq!(modes.chosen, "plan");
    assert_eq!(modes.choices.len(), 3);
    assert!(modes.choices[2].dangerous);
    assert!(agents.modes_of("codex").is_none());
}

#[test]
fn a_config_without_settings_gives_no_agents_here() {
    assert_eq!(Agents::from_config(&json!({})), Agents::default());
}

#[test]
fn a_machines_config_gives_each_agents_modes_and_the_one_it_chose() {
    let claude = claude();
    assert_eq!(claude.chosen, "plan");
    let values: Vec<&str> = claude.choices.iter().map(|choice| choice.value.as_str()).collect();
    assert_eq!(values, ["default", "accept edits", "auto", "plan", "skip permissions (dangerous)"]);
    assert!(claude.choices[4].dangerous);
    assert!(!claude.choices[3].dangerous);
    assert_eq!(codex().chosen, "default");
}

#[test]
fn a_machines_answer_without_settings_cannot_be_read() {
    assert!(serde_json::from_str::<Agents>("{}").is_err());
    assert!(serde_json::from_str::<Agents>("[1, 2]").is_err());
}

#[test]
fn an_agent_without_a_mode_setting_offers_no_modes() {
    assert!(there().modes_of("amp").is_none());
}

#[test]
fn the_mode_shown_is_the_pick_while_it_is_offered_else_the_chosen_one() {
    let claude = claude();
    assert_eq!(claude.shown(None), "plan");
    assert_eq!(claude.shown(Some("auto")), "auto");
    assert_eq!(claude.shown(Some("read only")), "plan");
    assert_eq!(claude.shown(Some("default")), "plan");
}

#[test]
fn a_mode_goes_only_when_picked_and_unlike_the_chosen_one() {
    let claude = claude();
    assert_eq!(claude.sent(None), None);
    assert_eq!(claude.sent(Some("plan")), None);
    assert_eq!(claude.sent(Some("auto")).as_deref(), Some("auto"));
}

#[test]
fn default_rests_and_is_never_sent_while_another_mode_is_chosen() {
    let claude = claude();
    assert!(claude.rests("default"));
    assert!(!claude.rests("auto"));
    assert!(claude.waits());
    assert_eq!(claude.sent(Some("default")), None);
}

#[test]
fn default_is_the_chosen_chip_and_sends_nothing_when_settings_choose_it() {
    let codex = codex();
    assert!(!codex.rests("default"));
    assert!(!codex.waits());
    assert_eq!(codex.shown(None), "default");
    assert_eq!(codex.sent(Some("default")), None);
    assert_eq!(codex.sent(Some("read only")).as_deref(), Some("read only"));
}

#[test]
fn a_pick_another_server_does_not_list_is_not_sent() {
    assert_eq!(claude().sent(Some("read only")), None);
    assert_eq!(codex().sent(Some("plan")), None);
}
