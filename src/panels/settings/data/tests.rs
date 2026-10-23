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
fn errors_lose_the_command_name() {
    let e = anyhow::anyhow!("cornercase config: error: `fetch_minutes` takes a whole number");
    assert_eq!(clean_error(&e), "`fetch_minutes` takes a whole number");
    let e = anyhow::anyhow!("cornercase config: Error: the token was refused");
    assert_eq!(clean_error(&e), "the token was refused");
    assert_eq!(clean_error(&anyhow::anyhow!("could not run")), "could not run");
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
