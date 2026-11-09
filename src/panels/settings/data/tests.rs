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
fn search_finds_a_setting_by_the_value_it_shows() {
    let d = sample();
    assert_eq!(search(&d, "5 min")[0].section.items, [Item::Setting(1)]);
}

#[test]
fn search_does_not_read_a_switchs_box_as_its_value() {
    assert_eq!(search(&sample(), "[x]"), Vec::<Found>::new());
}

#[test]
fn search_finds_a_setting_by_its_page() {
    let d = sample();
    let found = search(&d, "ui shown");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].page, "UI");
}

#[test]
fn search_finds_a_connection_by_the_state_it_shows() {
    let d = sample();
    let found = search(&d, "refused");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].page, "Remote");
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

#[test]
fn a_project_reads_its_build_folders_as_one_text() {
    let d = sample();
    assert_eq!(Script::Builds.of(&d.projects[0]), "node_modules target");
    assert_eq!(Script::Builds.of(&d.projects[1]), "");
}

#[test]
fn the_build_folders_are_saved_like_a_script() {
    let d = sample();
    assert_eq!(
        project_args(&d.projects[0], Script::Builds, "default"),
        ["config", "project", "--json", "--project", "4", "--builds=default"]
    );
}

#[test]
fn only_a_repository_has_build_folders() {
    let d = sample();
    assert_eq!(Script::all_of(&d.projects[0]), [Script::Setup, Script::Teardown, Script::Builds]);
    assert_eq!(Script::all_of(&d.projects[1]), [Script::Setup, Script::Teardown]);
}

#[test]
fn the_project_asked_for_comes_first_and_the_others_keep_their_order() {
    let projects: Vec<ProjectEntry> = [Some(1), None, Some(3), Some(4)]
        .into_iter()
        .map(|project| ProjectEntry { project, ..ProjectEntry::default() })
        .collect();
    let order = |first| -> Vec<Option<u64>> { shown_projects(&projects, first).iter().map(|e| e.project).collect() };
    assert_eq!(order(Some(3)), [Some(3), Some(1), None, Some(4)]);
    assert_eq!(order(None), [Some(1), None, Some(3), Some(4)]);
    assert_eq!(order(Some(9)), [Some(1), None, Some(3), Some(4)]);
}

fn chat(state: State, note: &str) -> Connection {
    Connection { kind: ConnectionKind::Chat, state, note: note.to_string(), ..Connection::default() }
}

#[test]
fn an_unpaired_chat_says_what_pairing_it_shows_without_the_tuis_key() {
    let row = chat(State::Missing, "enter shows a code to scan with your phone");
    assert_eq!(row.note_here(), "shows a code to scan with your phone");
    assert_eq!(row.pairing_offer(), Some("Pair…"));
}

#[test]
fn a_paired_chat_says_what_it_is_for_and_offers_another() {
    let row = chat(State::Connected, "agents message it while you are away · enter pairs another");
    assert_eq!(row.note_here(), "agents message it while you are away");
    assert_eq!(row.pairing_offer(), Some("Pair another…"));
}

#[test]
fn a_paired_chat_shows_its_name() {
    let row = Connection { account: Some("Ana".to_string()), ..chat(State::Connected, "") };
    assert_eq!(row.status(), ("connected as Ana".to_string(), Tone::Good));
}

#[test]
fn browsers_drop_the_tuis_key_and_offer_to_pair() {
    let none = Connection {
        kind: ConnectionKind::Browsers,
        count: Some(0),
        note: "enter forgets them all".to_string(),
        ..Connection::default()
    };
    assert_eq!(none.note_here(), "");
    assert_eq!(none.pairing_offer(), Some("Pair a browser…"));
    let some = Connection { count: Some(2), ..none };
    assert_eq!(some.pairing_offer(), Some("Pair another…"));
}

#[test]
fn a_tracker_or_a_bot_offers_no_pairing_and_keeps_only_its_own_note() {
    let tracker = Connection { note: "enter pastes one".to_string(), ..Connection::default() };
    assert_eq!(tracker.note_here(), "");
    assert_eq!(tracker.pairing_offer(), None);
    let bot = Connection { kind: ConnectionKind::Bot, note: "paste its token".to_string(), ..Connection::default() };
    assert_eq!(bot.note_here(), "paste its token");
    assert_eq!(bot.pairing_offer(), None);
}

fn shell() -> Described {
    parse(crate::test_util::fixture!("settings/shell.json")).expect("the shell sample parses")
}

fn refused() -> Described {
    parse(crate::test_util::fixture!("settings/shell-refused.json")).expect("the refused sample parses")
}

fn offered_values(setting: &Setting) -> Vec<String> {
    setting.offered().into_iter().map(|c| c.value).collect()
}

#[test]
fn a_text_setting_with_choices_takes_any_text_and_suggests_them() {
    let (d, sample) = (shell(), sample());
    assert!(d.settings[1].suggests() && d.settings[1].edited_as_text());
    assert!(!d.settings[2].suggests());
    assert!(!sample.settings[4].suggests());
}

#[test]
fn the_suggestions_start_with_the_default_then_follow_cornercases_order() {
    let setting = &shell().settings[1];
    let values = ["", "/bin/bash", "/bin/csh", "/bin/dash", "/bin/ksh", "/bin/sh", "/bin/tcsh", "/bin/zsh"];
    assert_eq!(offered_values(setting), values);
    let labels: Vec<String> = setting.offered().iter().map(|c| setting.offered_label(c)).collect();
    assert_eq!(labels[..2], ["default", "/bin/bash"]);
}

#[test]
fn a_suggested_path_is_shown_as_written() {
    let mut setting = shell().settings[1].clone();
    setting.choices = vec![Choice { value: "/opt/my_tools/nu".into(), ..Choice::default() }];
    let choice = &setting.offered()[1];
    assert_eq!(setting.offered_label(choice), "/opt/my_tools/nu");
}

#[test]
fn a_default_already_among_the_choices_is_not_listed_twice() {
    let mut setting = shell().settings[1].clone();
    setting.choices =
        vec![Choice { value: String::new(), ..Choice::default() }, Choice { value: "x".into(), ..Choice::default() }];
    assert_eq!(offered_values(&setting), ["", "x"]);
}

#[test]
fn a_choice_setting_offers_its_choices_with_spaces_for_underscores() {
    let d = sample();
    assert_eq!(d.settings[4].offered(), d.settings[4].choices);
    let mut setting = d.settings[4].clone();
    setting.choices = vec![Choice { value: "projects_on_top".into(), ..Choice::default() }];
    assert_eq!(setting.offered_label(&setting.offered()[0]), "projects on top");
}

#[test]
fn the_default_is_in_use_while_the_field_is_empty() {
    let mut setting = shell().settings[1].clone();
    assert_eq!(setting.in_use(), Some(0));
    for (value, at) in [("/bin/zsh", Some(7)), ("zsh", None), ("/usr/local/bin/fish", None)] {
        setting.value = Value::String(value.into());
        assert_eq!(setting.in_use(), at, "{value}");
    }
}

#[test]
fn a_text_setting_with_suggestions_is_never_drawn_as_segments() {
    let mut setting = shell().settings[1].clone();
    setting.choices.truncate(2);
    assert!(!setting.as_segments());
}

#[test]
fn a_shell_that_cannot_start_warns_and_one_that_can_does_not() {
    let (bad, good) = (refused(), shell());
    assert!(bad.settings[1].warns());
    assert_eq!(bad.settings[1].note, "cannot start: there is no program there; new panes start zsh");
    assert!(!good.settings[1].warns() && !bad.settings[2].warns());
}

#[test]
fn the_reset_arrow_names_the_default() {
    let (d, sample) = (refused(), sample());
    assert_eq!(reset_hint(&d.settings[1]), "Back to the default shell");
    assert_eq!(reset_hint(&sample.settings[0]), "Back to the default: ~/.cornercase/worktrees");
}

#[test]
fn the_reset_arrow_of_empty_arguments_names_the_default_empty() {
    let mut setting = shell().settings[2].clone();
    setting.value = Value::String("-l".into());
    assert_eq!(reset_hint(&setting), "Back to the default: empty");
}

#[test]
fn an_empty_shell_reads_as_cornercases_words_for_it() {
    let d = shell();
    assert_eq!(placeholder(&d.settings[1]), "default (zsh)");
    assert_eq!(placeholder(&d.settings[2]), "none");
}

#[test]
fn shell_arguments_that_look_like_options_are_values() {
    assert_eq!(set_args("shell.args", "-l"), ["config", "set", "--json", "--", "shell.args", "-l"]);
    let quoted = set_args("shell.args", "'/Users/me/My Dir/x' --login");
    assert_eq!(quoted.last().map(String::as_str), Some("'/Users/me/My Dir/x' --login"));
    assert_eq!(set_args("shell", "")[4..], ["shell", ""]);
}

#[test]
fn the_shell_and_its_arguments_are_rows_of_the_ui_pages_card() {
    let d = shell();
    let ui = sections(&d, "UI");
    assert_eq!(ui.len(), 1);
    assert_eq!(ui[0].title, "");
    assert_eq!(ui[0].items, [Item::Setting(0), Item::Setting(1), Item::Setting(2), Item::Setting(3)]);
}

#[test]
fn search_finds_the_shell_rows_by_name_suggestion_and_note() {
    let items = |d: &Described, query: &str| -> Vec<Item> {
        let found = search(d, query);
        assert_eq!(found.len(), 1, "{query}");
        assert_eq!(found[0].page, "UI");
        found[0].section.items.clone()
    };
    let d = shell();
    assert_eq!(items(&d, "shell"), [Item::Setting(1), Item::Setting(2)]);
    assert_eq!(items(&d, "tcsh"), [Item::Setting(1)]);
    assert_eq!(items(&d, "login"), [Item::Setting(2)]);
    assert_eq!(items(&refused(), "fish"), [Item::Setting(1)]);
}

#[test]
fn every_suggestion_is_saved_exactly_as_listed() {
    let setting = shell().settings[1].clone();
    for choice in setting.offered() {
        assert_eq!(choice.value.trim(), choice.value);
        assert_eq!(set_args(&setting.key, choice.value.trim()).last(), Some(&choice.value));
    }
    assert_eq!(setting.offered()[0].value, "");
}
