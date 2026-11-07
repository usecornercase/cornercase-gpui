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
fn what_the_rows_show_is_on_until_a_setting_says_otherwise() {
    let look = parse(r#"{"settings":[]}"#).expect("a look");
    assert!(look.model && look.context && look.counts && look.todo_dot && look.dim_inactive_panes);
    let default = Look::default();
    assert!(default.model && default.context && default.counts && default.todo_dot && default.dim_inactive_panes);
}

#[test]
fn each_switch_turns_off_on_its_own() {
    let text = r#"{"settings":[{"key":"model","value":false},{"key":"counts","value":false},
        {"key":"dim_inactive_panes","value":false}]}"#;
    let look = parse(text).expect("a look");
    assert!(!look.model && !look.counts && !look.dim_inactive_panes);
    assert!(look.context && look.todo_dot);
}

#[test]
fn the_settings_are_read_from_a_real_answer() {
    let look = parse(crate::test_util::fixture!("settings/config-ui.json")).expect("a look");
    assert!(!look.model && !look.context && !look.counts && !look.todo_dot && !look.dim_inactive_panes);
    assert_eq!(look.arrangement, Arrangement::ProjectsOnTop);
}

fn opus() -> PaneInfo {
    PaneInfo { model: Some("Opus 5.5".into()), context: Some(23), ..Default::default() }
}

#[test]
fn the_model_and_its_share_follow_their_switches() {
    let look = Look::default();
    assert_eq!(look.model_of(&opus()).as_deref(), Some("Opus 5.5"));
    assert_eq!(look.share_of(&opus()).as_deref(), Some("23%"));
    let off = Look { model: false, context: false, ..Look::default() };
    assert_eq!(off.model_of(&opus()), None);
    assert_eq!(off.share_of(&opus()), None);
}

#[test]
fn a_compacted_conversation_reads_where_the_percentage_goes() {
    let pane = PaneInfo { compacted: true, context: None, ..opus() };
    assert_eq!(Look::default().share_of(&pane).as_deref(), Some("compacted"));
}

#[test]
fn compacted_follows_the_context_switch() {
    let pane = PaneInfo { compacted: true, context: None, ..opus() };
    assert_eq!(Look { context: false, ..Look::default() }.share_of(&pane), None);
}

#[test]
fn a_figure_wins_over_compacted() {
    let pane = PaneInfo { compacted: true, ..opus() };
    assert_eq!(Look::default().share_of(&pane).as_deref(), Some("23%"));
}

#[test]
fn inactive_panes_fade_only_with_their_switch_on() {
    assert!(Look::default().fades(true, false));
    assert!(!Look { dim_inactive_panes: false, ..Look::default() }.fades(true, false));
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

#[test]
fn a_project_with_a_setup_script_is_known_by_its_folder() {
    let look = parse(crate::test_util::fixture!("settings/config.json")).expect("a look");
    assert_eq!(look.setups, ["/code/shop"]);
}

#[test]
fn a_blank_setup_script_counts_as_none() {
    let look = parse(r#"{"projects":[{"folder":"/code/shop","setup":"  \n"}]}"#).expect("a look");
    assert_eq!(look.setups, Vec::<String>::new());
}
