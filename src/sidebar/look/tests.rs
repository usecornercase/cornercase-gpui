use super::*;

#[test]
fn reads_the_arrangement_tabs_and_agents_section() {
    let text = r#"{"revision":4,"settings":[{"key":"sidebar","value":"side_by_side"},
        {"key":"tabs","value":"sidebar"},{"key":"agents_section","value":true}]}"#;
    let look = parse(text).expect("a look");
    assert_eq!(look.arrangement, Arrangement::SideBySide);
    assert!(look.tabs_in_sidebar);
    assert!(look.agents_section);
    assert_eq!(look.revision, 4);
}

#[test]
fn each_arrangement_has_its_value() {
    let of = |v: &str| parse(&format!(r#"{{"settings":[{{"key":"sidebar","value":"{v}"}}]}}"#)).expect("a look");
    assert_eq!(of("projects_on_top").arrangement, Arrangement::ProjectsOnTop);
    assert_eq!(of("workspaces_on_top").arrangement, Arrangement::WorkspacesOnTop);
    assert_eq!(of("tree").arrangement, Arrangement::Tree);
}

#[test]
fn missing_settings_keep_the_tree_with_tabs_on_top() {
    let look = parse(r#"{"settings":[{"key":"sidebar","value":"something_new"}]}"#).expect("a look");
    assert_eq!(look.arrangement, Arrangement::Tree);
    assert!(!look.tabs_in_sidebar);
    assert!(!look.agents_section);
}

#[test]
fn the_account_switches_are_on_unless_turned_off() {
    let look = parse(r#"{"settings":[]}"#).expect("a look");
    assert!(look.account_mark && look.account_switcher);
    let text = r#"{"settings":[{"key":"account_mark","value":false},{"key":"account_switcher","value":false}]}"#;
    let look = parse(text).expect("a look");
    assert!(!look.account_mark && !look.account_switcher);
}

#[test]
fn the_account_picker_is_on_unless_turned_off() {
    assert!(parse(r#"{"settings":[]}"#).expect("a look").account_picker);
    let text = r#"{"settings":[{"key":"account_picker","value":false}]}"#;
    assert!(!parse(text).expect("a look").account_picker);
}

#[test]
fn an_agent_chosen_in_the_settings_is_the_default() {
    let text = r#"{"settings":[{"key":"agent","value":"claude"}]}"#;
    assert_eq!(parse(text).expect("a look").default_agent.as_deref(), Some("claude"));
}

#[test]
fn auto_names_no_default_agent() {
    let text = r#"{"settings":[{"key":"agent","value":"auto"}]}"#;
    assert_eq!(parse(text).expect("a look").default_agent, None);
}
