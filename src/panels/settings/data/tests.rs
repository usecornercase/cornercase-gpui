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

fn phone_off() -> Described {
    parse(crate::test_util::fixture!("settings/phone-app-off.json")).expect("the off answer parses")
}

fn phone_paired() -> Described {
    parse(crate::test_util::fixture!("settings/phone-app-paired.json")).expect("the paired answer parses")
}

fn phone_older() -> Described {
    parse(crate::test_util::fixture!("settings/phone-app-older.json")).expect("the older answer parses")
}

fn apps(d: &Described) -> Vec<usize> {
    (0..d.connections.len()).filter(|&i| d.connections[i].kind == ConnectionKind::App).collect()
}

fn app(account: Option<&str>, note: &str) -> Connection {
    Connection {
        key: "app.0011223344556677".into(),
        kind: ConnectionKind::App,
        label: "paired app".into(),
        note: note.into(),
        state: State::Connected,
        account: account.map(str::to_string),
        removable: true,
        ..Connection::default()
    }
}

fn app_status(cs: &[Connection], at: usize) -> Option<String> {
    let row = row_status(cs, at);
    row.named.then_some(row.text)
}

const DAY: &str = "paired 2026-11-14 · enter forgets it";

#[test]
fn a_paired_phone_app_is_read_as_an_app() {
    let d = phone_paired();
    let at = apps(&d);
    assert_eq!(at.len(), 3);
    for i in at {
        let c = &d.connections[i];
        assert_eq!((c.state, c.removable, c.label.as_str()), (State::Connected, true, "paired app"));
    }
}

#[test]
fn the_phone_app_is_offered_only_while_its_switch_is_on() {
    assert_eq!(app_offer(&phone_off()), None);
    let mut on = phone_off();
    on.settings.iter_mut().filter(|s| s.key == PHONE_APP).for_each(|s| s.value = Value::Bool(true));
    assert_eq!(app_offer(&on), Some("Pair…"));
    assert_eq!(app_offer(&phone_paired()), Some("Pair another…"));
}

#[test]
fn a_server_that_lists_no_phone_app_offers_none() {
    assert_eq!(app_offer(&phone_older()), None);
    assert_eq!(app_offer(&sample()), None);
}

#[test]
fn a_paired_app_shows_its_name_as_cornercase_gives_it() {
    let cs = [app(Some("cornercase-mobile/0.1.0 (Ana's iPhone)"), DAY)];
    assert_eq!(app_status(&cs, 0).as_deref(), Some("cornercase-mobile/0.1.0 (Ana's iPhone)"));
    assert_eq!(row_status(&cs, 0).tone, Tone::Good);
}

#[test]
fn an_app_that_sent_no_name_shows_cornercases_word_for_it() {
    let d = phone_paired();
    let nameless = apps(&d).into_iter().find(|&i| d.connections[i].account.as_deref() == Some("phone app"));
    assert_eq!(app_status(&d.connections, nameless.expect("the nameless app")).as_deref(), Some("phone app"));
}

#[test]
fn two_apps_that_would_read_alike_are_numbered_in_the_order_cornercase_lists_them() {
    let d = phone_paired();
    let at = apps(&d);
    assert_eq!(app_status(&d.connections, at[0]).as_deref(), Some("cornercase-mobile/0.1.0 (Ana's iPhone) · 1 of 2"));
    assert_eq!(app_status(&d.connections, at[2]).as_deref(), Some("cornercase-mobile/0.1.0 (Ana's iPhone) · 2 of 2"));
    assert_eq!(app_status(&d.connections, at[1]).as_deref(), Some("phone app"));
}

#[test]
fn apps_of_one_name_paired_on_different_days_are_told_by_their_date() {
    let cs = [app(Some("Ana"), "paired 2026-11-13 · enter forgets it"), app(Some("Ana"), DAY)];
    assert_eq!((alike(&cs, 0), alike(&cs, 1)), (None, None));
    assert_eq!(app_status(&cs, 1).as_deref(), Some("Ana"));
}

#[test]
fn an_app_whose_name_is_missing_says_it_is_connected() {
    let cs = [app(None, DAY)];
    assert_eq!(app_status(&cs, 0), None);
    let row = row_status(&cs, 0);
    assert_eq!((row.text.as_str(), row.tone), ("connected", Tone::Good));
}

#[test]
fn a_paired_apps_note_keeps_the_day_it_paired() {
    assert_eq!(app(Some("Ana"), DAY).note_here(), "paired 2026-11-14");
}

#[test]
fn a_row_that_is_not_an_app_has_no_name_of_one() {
    let chat = Connection {
        kind: ConnectionKind::Chat,
        state: State::Connected,
        account: Some("Ana".into()),
        ..Connection::default()
    };
    let cs = [chat];
    assert_eq!(app_status(&cs, 0), None);
    assert_eq!(alike(&cs, 0), None);
    assert_eq!(cs[0].status().0, "connected as Ana");
}

#[test]
fn the_trash_of_a_paired_app_says_forget() {
    let of = |kind| Connection { kind, ..Connection::default() };
    assert_eq!(of(ConnectionKind::App).removal_hint(), "Forget…");
    assert_eq!(of(ConnectionKind::Chat).removal_hint(), "Forget…");
    assert_eq!(of(ConnectionKind::Browsers).removal_hint(), "Forget every paired browser…");
    assert_eq!(of(ConnectionKind::Tracker).removal_hint(), "Remove…");
}

fn titles(d: &Described, page: &str) -> Vec<String> {
    sections(d, page).into_iter().map(|s| s.title).collect()
}

#[test]
fn the_phone_app_section_follows_the_browsers_with_nothing_paired() {
    assert_eq!(titles(&phone_off(), "Remote"), ["Browser", "Phone app", "Telegram", "Discord"]);
}

#[test]
fn the_phone_app_section_keeps_its_place_once_an_app_is_paired() {
    let d = phone_paired();
    assert_eq!(titles(&d, "Remote"), ["Browser", "Phone app", "Telegram", "Discord"]);
    let section = &sections(&d, "Remote")[1];
    let switch = d.settings.iter().position(|s| s.key == PHONE_APP).expect("the switch");
    let expected: Vec<Item> =
        std::iter::once(Item::Setting(switch)).chain(apps(&d).into_iter().map(Item::Connection)).collect();
    assert_eq!(section.items, expected);
}

#[test]
fn a_section_only_settings_name_with_none_before_it_still_goes_last() {
    let d: Described = serde_json::from_value(serde_json::json!({
        "settings": [{"key": "a", "page": "P", "section": "Only settings"}],
        "connections": [{"key": "c", "kind": "tracker", "page": "P", "section": "Accounts"}]
    }))
    .expect("parses");
    assert_eq!(titles(&d, "P"), ["Accounts", "Only settings"]);
}

#[test]
fn an_older_servers_remote_page_keeps_its_sections() {
    assert_eq!(titles(&phone_older(), "Remote"), ["Browser", "Telegram", "Discord"]);
}

#[test]
fn search_for_phone_app_finds_the_switch_and_every_paired_app() {
    let found = search(&phone_paired(), "phone app");
    let phone: Vec<&Found> = found.iter().filter(|f| f.section.title == "Phone app").collect();
    assert_eq!(phone.len(), 1);
    assert_eq!(phone[0].section.items.len(), 4);
}

#[test]
fn search_for_pair_finds_the_paired_apps() {
    let d = phone_paired();
    let found = search(&d, "pair");
    let items: Vec<&Item> = found.iter().flat_map(|f| f.section.items.iter()).collect();
    for i in apps(&d) {
        assert!(items.contains(&&Item::Connection(i)));
    }
}

#[test]
fn an_app_whose_name_is_blank_says_it_is_connected_like_one_with_none() {
    let cs = [app(Some(""), DAY), app(Some("  "), DAY)];
    assert_eq!((app_status(&cs, 0), app_status(&cs, 1)), (None, None));
    assert_eq!((cs[0].app_name(), cs[1].app_name()), (None, None));
    assert_eq!((row_status(&cs, 0).text, row_status(&cs, 1).text), ("connected".to_string(), "connected".to_string()));
}

#[test]
fn search_finds_the_number_a_row_of_two_alike_apps_shows() {
    let d = phone_paired();
    let found = search(&d, "1 of 2");
    let items: Vec<&Item> = found.iter().flat_map(|f| f.section.items.iter()).collect();
    let at = apps(&d);
    assert!(items.contains(&&Item::Connection(at[0])));
    assert!(!items.contains(&&Item::Connection(at[1])));
}

fn seen(boot: &str, revision: u64) -> Seen<'_> {
    Seen { boot, revision }
}

#[test]
fn a_revision_that_comes_while_the_settings_load_is_read_once_that_load_ends() {
    assert!(behind(Some(seen("b1", 7)), seen("b1", 8)));
}

#[test]
fn an_answer_level_with_the_session_reads_nothing_more() {
    assert!(!behind(Some(seen("b1", 8)), seen("b1", 8)));
}

#[test]
fn an_answer_ahead_of_the_session_reads_nothing() {
    assert!(!behind(Some(seen("b1", 9)), seen("b1", 8)));
}

#[test]
fn an_answer_from_another_boot_is_read_again() {
    assert!(behind(Some(seen("b1", 9)), seen("b2", 3)));
}

#[test]
fn with_no_session_nothing_is_read() {
    assert!(!behind(Some(seen("b1", 7)), seen("", 0)));
}

#[test]
fn with_nothing_held_nothing_is_behind() {
    assert!(!behind(None, seen("b1", 8)));
}

fn asked(boot: &str, revision: u64, view: Option<u64>) -> Asked {
    Asked { boot: boot.to_string(), revision, view }
}

fn sheet<'a>(held: Option<Seen<'a>>, failed: Option<&'a Asked>) -> Sheet<'a> {
    Sheet { open: true, reading: false, held, failed }
}

#[test]
fn a_read_that_failed_asks_nothing_while_the_session_says_what_it_said_when_the_read_was_sent() {
    let failed = asked("b1", 8, Some(3));
    assert!(!reads_again(sheet(None, Some(&failed)), seen("b1", 8), Some(3)));
}

#[test]
fn a_read_that_failed_asks_again_once_the_server_starts_again() {
    let failed = asked("b1", 8, Some(3));
    assert!(reads_again(sheet(None, Some(&failed)), seen("b2", 1), Some(3)));
}

#[test]
fn a_read_that_failed_asks_again_once_the_settings_change() {
    let failed = asked("b1", 8, Some(3));
    assert!(reads_again(sheet(None, Some(&failed)), seen("b1", 9), Some(3)));
}

#[test]
fn a_read_sent_before_the_window_connected_asks_again_once_it_has() {
    let failed = asked("", 0, None);
    assert!(reads_again(sheet(None, Some(&failed)), seen("b1", 1), Some(1)));
}

#[test]
fn a_read_that_failed_asks_again_once_the_window_connects_again() {
    let failed = asked("b1", 8, Some(3));
    assert!(reads_again(sheet(None, Some(&failed)), seen("b1", 8), Some(5)));
}

#[test]
fn a_read_that_failed_asks_nothing_while_the_window_is_not_connected() {
    let failed = asked("b1", 8, Some(3));
    assert!(!reads_again(sheet(None, Some(&failed)), seen("b1", 8), None));
}

#[test]
fn a_read_that_failed_while_an_answer_is_held_waits_for_the_session_to_move() {
    let failed = asked("b1", 8, Some(3));
    assert!(!reads_again(sheet(Some(seen("b1", 7)), Some(&failed)), seen("b1", 8), Some(3)));
}

#[test]
fn a_read_that_failed_while_an_answer_is_held_asks_again_once_the_session_moves_past_it() {
    let failed = asked("b1", 8, Some(3));
    assert!(reads_again(sheet(Some(seen("b1", 7)), Some(&failed)), seen("b1", 9), Some(3)));
}

#[test]
fn a_read_that_failed_while_an_answer_is_held_asks_nothing_when_the_answer_is_ahead_of_the_session() {
    let failed = asked("b1", 7, Some(3));
    assert!(!reads_again(sheet(Some(seen("b1", 9)), Some(&failed)), seen("b1", 8), Some(5)));
}

#[test]
fn nothing_held_and_nothing_failed_asks_nothing_beyond_the_opens_own_read() {
    assert!(!reads_again(sheet(None, None), seen("b1", 8), Some(3)));
}

#[test]
fn a_held_answer_with_no_failure_reads_again_as_behind_says() {
    let cases = [
        (seen("b1", 7), seen("b1", 8)),
        (seen("b1", 8), seen("b1", 8)),
        (seen("b1", 9), seen("b1", 8)),
        (seen("b1", 9), seen("b2", 3)),
        (seen("b1", 7), seen("", 0)),
    ];
    let again: Vec<bool> =
        cases.iter().map(|(held, now)| reads_again(sheet(Some(*held), None), *now, Some(3))).collect();
    assert_eq!(again, [true, false, false, true, false]);
}

#[test]
fn a_read_in_flight_is_never_joined_by_a_second() {
    let failed = asked("b1", 8, Some(3));
    let reading = |held, failed| Sheet { reading: true, ..sheet(held, failed) };
    assert!(!reads_again(reading(Some(seen("b1", 7)), None), seen("b1", 8), Some(3)));
    assert!(!reads_again(reading(None, Some(&failed)), seen("b2", 1), Some(3)));
}

#[test]
fn a_closed_sheet_asks_nothing() {
    let failed = asked("b1", 8, Some(3));
    let closed = |held, failed| Sheet { open: false, ..sheet(held, failed) };
    assert!(!reads_again(closed(Some(seen("b1", 7)), None), seen("b1", 8), Some(3)));
    assert!(!reads_again(closed(None, Some(&failed)), seen("b2", 1), Some(3)));
}

fn described() -> Described {
    Described::default()
}

const FIVE: [&str; 5] = [APPEARANCE, LINKS, "Worktrees", PROJECTS, SHARE];

#[test]
fn the_appearance_page_shows_with_no_query_whatever_the_settings() {
    let d = described();
    for answer in [Answer::Held(&d), Answer::Reading, Answer::Failed("why")] {
        assert_eq!(shown(APPEARANCE, false, answer), Shown::Appearance);
    }
}

#[test]
fn a_search_typed_on_the_appearance_page_shows_the_results() {
    let d = described();
    assert_eq!(shown(APPEARANCE, true, Answer::Held(&d)), Shown::Results(&d));
}

#[test]
fn the_links_page_shows_from_the_answer_once_it_is_held() {
    let d = described();
    assert_eq!(shown(LINKS, false, Answer::Held(&d)), Shown::Page(&d));
}

#[test]
fn the_links_page_works_with_no_answer() {
    assert_eq!(shown(LINKS, false, Answer::Reading), Shown::OwnLinks);
    assert_eq!(shown(LINKS, false, Answer::Failed("why")), Shown::OwnLinks);
}

#[test]
fn a_page_of_cornercase_shows_from_the_answer() {
    let d = described();
    for page in ["Worktrees", PROJECTS, SHARE] {
        assert_eq!(shown(page, false, Answer::Held(&d)), Shown::Page(&d));
    }
}

#[test]
fn a_page_of_cornercase_says_its_settings_are_being_read() {
    for page in ["Worktrees", PROJECTS, SHARE] {
        assert_eq!(shown(page, false, Answer::Reading), Shown::Reading);
    }
}

#[test]
fn a_page_of_cornercase_says_why_its_settings_could_not_be_read() {
    for page in ["Worktrees", PROJECTS, SHARE] {
        assert_eq!(shown(page, false, Answer::Failed("why")), Shown::Failed("why"));
    }
}

#[test]
fn a_search_shows_the_same_results_whichever_page_was_open() {
    let d = described();
    for page in FIVE {
        assert_eq!(shown(page, true, Answer::Held(&d)), Shown::Results(&d), "{page}");
    }
}

#[test]
fn a_search_while_the_settings_are_read_offers_the_pages_that_match() {
    for page in FIVE {
        assert_eq!(shown(page, true, Answer::Reading), Shown::ResultsReading, "{page}");
    }
}

#[test]
fn a_search_with_the_settings_failed_says_why_and_offers_the_pages_that_match() {
    for page in FIVE {
        assert_eq!(shown(page, true, Answer::Failed("why")), Shown::ResultsFailed("why"), "{page}");
    }
}

#[test]
fn an_answer_older_than_what_the_sheet_holds_is_not_applied() {
    assert!(older(Some(seen("b1", 9)), seen("b1", 8)));
}

#[test]
fn an_answer_level_with_what_the_sheet_holds_is_applied() {
    assert!(!older(Some(seen("b1", 9)), seen("b1", 9)));
}

#[test]
fn an_answer_newer_than_what_the_sheet_holds_is_applied() {
    assert!(!older(Some(seen("b1", 8)), seen("b1", 9)));
}

#[test]
fn an_answer_asked_under_another_boot_is_applied_whatever_its_revision() {
    assert!(!older(Some(seen("b1", 9)), seen("b2", 1)));
}

#[test]
fn an_answer_with_no_revision_is_older_than_anything_held() {
    assert!(older(Some(seen("b1", 1)), seen("b1", 0)));
}

#[test]
fn with_nothing_held_every_answer_is_applied() {
    assert!(!older(None, seen("b1", 3)));
}

#[test]
fn settings_search_finds_the_appearance_page_by_every_word_the_global_search_finds_it_by() {
    let pages = [APPEARANCE.to_string()];
    for query in ["theme", "dark", "Gruvbox", "nord", "rosé pine", "contrast", "colours", "blur", "color"] {
        assert!(crate::panels::settings::appearance_found(query, "Nord").is_some(), "{query}");
        assert_eq!(pages_matching(&pages, query), [APPEARANCE], "{query}");
    }
}
