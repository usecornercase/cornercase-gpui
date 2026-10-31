use super::*;

fn sample() -> Described {
    parse(crate::test_util::fixture!("settings/config.json")).expect("the sample parses")
}

#[test]
fn parses_every_part() {
    let d = sample();
    assert_eq!((d.revision, d.settings.len()), (7, 11));
    assert_eq!((d.settings[1].kind, d.settings[10].kind), (Kind::Number, Kind::Other));
    assert_eq!((d.connections[0].state, d.connections[2].kind), (State::Connected, ConnectionKind::Browsers));
    assert_eq!((d.projects[0].project, d.projects[1].project), (Some(4), None));
    assert!(parse("{}").is_ok());
    assert!(parse("not json").is_err());
}

#[test]
fn values_read_as_set_takes_them() {
    let d = sample();
    let texts: Vec<String> = [0, 1, 3, 8].iter().map(|&i| d.settings[i].text()).collect();
    assert_eq!(texts, ["~/wt", "5", "all,github", "true"]);
    assert_eq!(d.settings[3].items(), vec!["all", "github"]);
    assert_eq!((d.settings[8].switch_text(), d.settings[9].switch_text()), ("shown", "off"));
    let flags = [
        d.settings[8].on(),
        d.settings[0].is_default(),
        d.settings[1].is_default(),
        d.settings[4].as_segments(),
        d.settings[5].as_segments(),
    ];
    assert_eq!(flags, [true, false, true, true, false]);
}

#[test]
fn set_passes_values_that_look_like_options() {
    assert_eq!(
        set_args("agents.amp.extra", "--model x"),
        ["config", "set", "--json", "--", "agents.amp.extra", "--model x"]
    );
}

#[test]
fn project_commands_name_the_project_or_its_folder() {
    let d = sample();
    assert_eq!(
        project_args(&d.projects[0], Script::Setup, "-x"),
        ["config", "project", "--json", "--project", "4", "--setup=-x"]
    );
    assert_eq!(
        project_args(&d.projects[1], Script::Teardown, ""),
        ["config", "project", "--json", "--teardown=", "--", "/code/gone"]
    );
}

#[test]
fn pages_follow_the_tui_order() {
    let d = sample();
    assert_eq!(pages(&d), ["Worktrees", "Agents", "Issues", "UI", "Remote", "Later"]);
}

#[test]
fn sections_group_connections_and_agents() {
    let d = sample();
    let issues = sections(&d, "Issues");
    let titles: Vec<&str> = issues.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(titles, ["Accounts", "Jira", "Sources shown"]);
    assert_eq!(issues[1].items, [Item::Setting(2), Item::Connection(1)]);

    let agents = sections(&d, "Agents");
    assert_eq!(
        agents[1].items,
        [
            Item::Agent { kind: "claude".into(), mode: Some(5), extra: Some(6) },
            Item::Agent { kind: "amp".into(), mode: None, extra: Some(7) },
        ]
    );

    let remote = sections(&d, "Remote");
    assert_eq!(remote[0].items, [Item::Setting(9), Item::Connection(2)]);
    assert_eq!(remote[1].title, "Telegram");
}

#[test]
fn lists_toggle_and_move() {
    let items: Vec<String> = vec!["all".into(), "github".into()];
    assert_eq!(list_toggle(&items, "linear"), ["all", "github", "linear"]);
    assert_eq!(list_toggle(&items, "all"), ["github"]);
    assert_eq!(list_move(&items, 1, -1), ["github", "all"]);
    assert_eq!(list_move(&items, 0, -1), ["all", "github"]);
    assert_eq!(list_move(&items, 1, 1), ["all", "github"]);
}

#[test]
fn connections_say_their_state() {
    let d = sample();
    assert_eq!(d.connections[0].status(), ("connected as me@example.com".into(), Tone::Good));
    assert_eq!(d.connections[1].status(), ("not set".into(), Tone::Quiet));
    assert_eq!(d.connections[2].status(), ("2 paired".into(), Tone::Good));
    assert_eq!(d.connections[3].status(), ("refused · from $TELEGRAM_TOKEN".into(), Tone::Bad));
    assert!(d.connections[0].takes_secret());
    assert!(!d.connections[3].takes_secret());
    assert!(!d.connections[2].takes_secret());
}

#[test]
fn numbers_are_whole() {
    assert!(is_number("10"));
    assert!(is_number(" 0 "));
    assert!(!is_number(""));
    assert!(!is_number("-1"));
    assert!(!is_number("1.5"));
}

#[test]
fn exports_parse() {
    let e = parse_export(r#"{"text":"version = 1\n","settings":49,"repositories":1,"left_out":["jira_email"]}"#)
        .expect("the export parses");
    assert_eq!(e.settings, 49);
    assert_eq!(e.left_out, ["jira_email"]);
    assert_eq!(e.warnings, Vec::<String>::new());
}

#[test]
fn a_number_keeps_the_unit_cornercase_shows_after_it() {
    let d = sample();
    assert_eq!(d.settings[1].unit(), Some("min".to_string()));
    let off = Setting { value: serde_json::json!(0), shown: "off".into(), ..d.settings[1].clone() };
    assert_eq!(off.unit(), None);
    assert_eq!(d.settings[0].unit(), None);
}

#[test]
fn a_field_hint_shows_only_when_it_says_more_than_the_label() {
    let d = sample();
    assert_eq!(d.settings[0].hint(), None);
    let branch = Setting { field: Some("branch, with {key} {slug}".into()), ..d.settings[0].clone() };
    assert_eq!(branch.hint(), Some("branch, with {key} {slug}"));
    let same_as_note = Setting { field: Some("n".into()), ..d.settings[0].clone() };
    assert_eq!(same_as_note.hint(), None);
}

#[test]
fn sources_list_the_shown_ones_in_order_then_the_hidden() {
    let d = sample();
    assert_eq!(
        sources(&d.settings[3]),
        [("all".to_string(), true), ("github".to_string(), true), ("linear".to_string(), false)]
    );
}

#[test]
fn a_dangerous_choice_in_use_is_risky() {
    let d = sample();
    assert!(!d.settings[5].risky_now());
    let skipping = Setting { value: serde_json::json!("skip permissions (dangerous)"), ..d.settings[5].clone() };
    assert!(skipping.risky_now());
    assert_eq!(spoken("projects_on_top"), "projects on top");
}

#[test]
fn a_refused_token_marks_its_page() {
    let d = sample();
    assert!(alerts(&d, "Remote"));
    assert!(!alerts(&d, "Issues"));
}

#[test]
fn search_finds_settings_by_label_key_section_and_choice() {
    let d = sample();
    let pages_of = |query: &str| -> Vec<String> { search(&d, query).iter().map(|f| f.page.clone()).collect() };
    assert_eq!(pages_of("JIRA"), ["Issues"]);
    assert_eq!(search(&d, "jira")[0].section.items, [Item::Setting(2), Item::Connection(1)]);
    assert_eq!(pages_of("fetch_minutes"), ["Worktrees"]);
    assert_eq!(pages_of("claude"), ["Agents", "Agents"]);
    assert_eq!(search(&d, "   "), Vec::<Found>::new());
    assert_eq!(search(&d, "nothing like this"), Vec::<Found>::new());
}

#[test]
fn every_word_of_the_search_must_match() {
    let d = sample();
    let found = search(&d, "claude extra");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].section.items, [Item::Agent { kind: "claude".into(), mode: Some(5), extra: Some(6) }]);
    assert_eq!(count(&found, "Agents"), 1);
    assert_eq!(count(&found, "UI"), 0);
}

#[test]
fn pages_match_by_name_or_what_they_hold() {
    let pages: Vec<String> = ["Worktrees", "Projects", "Share"].map(String::from).to_vec();
    assert_eq!(pages_matching(&pages, "teardown"), ["Projects"]);
    assert_eq!(pages_matching(&pages, "export"), ["Share"]);
    assert_eq!(pages_matching(&pages, ""), Vec::<String>::new());
}

#[test]
fn an_empty_field_reads_as_what_empty_means() {
    let d = sample();
    assert_eq!(placeholder(&d.settings[6]), "none");
    let per_tracker = Setting { shown: "per tracker".into(), ..d.settings[6].clone() };
    assert_eq!(placeholder(&per_tracker), "per tracker");
    assert_eq!(placeholder(&d.settings[0]), "worktrees folder");
}

#[test]
fn a_default_reads_as_the_sheet_shows_it() {
    let d = sample();
    assert_eq!(default_shown(&d.settings[0]), "~/.cornercase/worktrees");
    assert_eq!(default_shown(&d.settings[8]), "on");
    assert_eq!(default_shown(&d.settings[6]), "empty");
    let layout = Setting { default: serde_json::json!("projects_on_top"), ..d.settings[4].clone() };
    assert_eq!(default_shown(&layout), "projects on top");
}

#[test]
fn a_section_holding_one_setting_of_its_name_has_no_caption() {
    let d = sample();
    let sources = Section { title: "Sources".into(), items: vec![Item::Setting(3)] };
    assert_eq!(caption(&d, &sources), "");
    let jira = &sections(&d, "Issues")[1];
    assert_eq!(caption(&d, jira), "Jira");
}
