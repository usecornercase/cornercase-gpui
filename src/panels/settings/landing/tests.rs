use super::*;
use crate::test_util::fixture;

fn described() -> Described {
    serde_json::from_value(serde_json::json!({
        "settings": [
            {"key": "fetch_minutes", "page": "Worktrees", "section": "", "label": "fetch branches every"},
            {"key": "sound", "page": "UI", "section": "", "label": "sound"},
            {"key": "issue_tabs", "page": "Issues", "section": "Sources shown", "label": "sources shown"},
            {"key": "agents.claude.mode", "page": "Agents", "section": "How each agent starts"},
            {"key": "agents.claude.extra", "page": "Agents", "section": "How each agent starts"},
            {"key": "remote", "page": "Remote", "section": "Browser", "label": "remote access"}
        ],
        "connections": [
            {"key": "linear", "kind": "tracker", "page": "Issues", "section": "Accounts", "label": "Linear API key"},
            {"key": "browsers", "kind": "browsers", "page": "Remote", "section": "Browser"},
            {"key": "discord", "kind": "bot", "page": "Remote", "section": "Discord", "label": "Discord bot"}
        ]
    }))
    .expect("the settings parse")
}

fn found(text: &str, title: &str) -> FoundSetting {
    let rows = parse_found(text).expect("the answer parses");
    rows.into_iter().find(|row| row.title == title).expect("a row with that title")
}

fn row_titled(key: &str, section: &str, label: &str) -> FoundSetting {
    FoundSetting {
        title: format!("Remote › {label}"),
        page: "Remote".into(),
        section: section.into(),
        label: label.into(),
        key: Some(key.into()),
        ..FoundSetting::default()
    }
}

fn key_of(d: &Described, spot: &Spot) -> String {
    match spot.item.as_ref().expect("a row to land on") {
        Item::Setting(i) => d.settings[*i].key.clone(),
        Item::Connection(i) => d.connections[*i].key.clone(),
        Item::Agent { kind, .. } => kind.clone(),
    }
}

#[test]
fn a_setting_lands_on_its_own_row() {
    let d = described();
    let spot = spot(&d, &found(fixture!("settings/search/sound.json"), "UI › sound"));
    assert_eq!(key_of(&d, &spot), "sound");
    assert_eq!(spot.notice, None);
}

#[test]
fn an_agents_mode_lands_on_the_agents_row_of_the_table() {
    let d = described();
    let spot = spot(&d, &found(fixture!("settings/search/claude.json"), "Agents › claude"));
    assert_eq!(key_of(&d, &spot), "claude");
}

#[test]
fn a_connection_lands_on_its_row() {
    let d = described();
    let spot = spot(&d, &found(fixture!("settings/search/accounts.json"), "Issues › Linear API key"));
    assert_eq!(key_of(&d, &spot), "linear");
}

#[test]
fn a_row_not_shown_yet_lands_on_the_first_of_its_section_saying_what_it_needs() {
    let d = described();
    let spot = spot(&d, &found(fixture!("settings/search/forum.json"), "Remote › forum mode"));
    assert_eq!(key_of(&d, &spot), "discord");
    assert_eq!(spot.notice.as_deref(), Some("Forum mode shows once a phone is paired"));
}

#[test]
fn a_row_cornercase_calls_hidden_that_this_sheet_draws_lands_on_it_with_no_notice() {
    let d = described();
    let hidden = FoundSetting {
        hidden: true,
        note: "shows once a browser is paired".into(),
        ..row_titled("browsers", "Browser", "paired browsers")
    };
    let spot = spot(&d, &hidden);
    assert_eq!(key_of(&d, &spot), "browsers");
    assert_eq!(spot.notice, None);
}

#[test]
fn a_tab_opens_its_page_with_nothing_marked() {
    let tab = FoundSetting { title: "UI".into(), page: "UI".into(), label: "UI".into(), ..FoundSetting::default() };
    assert!(tab.is_tab());
    assert_eq!(spot(&described(), &tab), Spot::default());
}

#[test]
fn a_row_without_a_key_lands_on_the_first_of_its_section() {
    let d = described();
    let another = FoundSetting {
        title: "Agents › + another agent…".into(),
        page: "Agents".into(),
        section: "How each agent starts".into(),
        label: "+ another agent…".into(),
        ..FoundSetting::default()
    };
    let spot = spot(&d, &another);
    assert_eq!(key_of(&d, &spot), "claude");
    assert_eq!(spot.notice, None);
}

#[test]
fn a_row_this_sheet_does_not_draw_says_where_it_is() {
    let tour = FoundSetting {
        title: "UI › start the tour".into(),
        page: "UI".into(),
        label: "start the tour".into(),
        ..FoundSetting::default()
    };
    let spot = spot(&described(), &tour);
    assert_eq!(spot.item, None);
    assert_eq!(spot.notice.as_deref(), Some("Start the tour is in cornercase's own window, Settings › UI."));
}

#[test]
fn a_page_this_window_does_not_have_says_where_it_is() {
    let later = FoundSetting {
        title: "Later › a row".into(),
        page: "Later".into(),
        label: "a row".into(),
        ..FoundSetting::default()
    };
    let spot = spot(&described(), &later);
    assert_eq!(spot.item, None);
    assert_eq!(spot.notice.as_deref(), Some("A row is in cornercase's own window, Settings › Later."));
}

#[test]
fn a_tab_with_a_sheet_of_its_own_opens_that_sheet() {
    assert_eq!(sheet_of("Accounts"), Some(PanelKind::Accounts));
    assert_eq!(sheet_of("Flows"), Some(PanelKind::Flows));
    assert_eq!(sheet_of("Plugins"), Some(PanelKind::Plugins));
    assert_eq!(sheet_of("Machines"), Some(PanelKind::Machines));
}

#[test]
fn a_page_of_settings_named_as_a_sheet_stays_in_settings() {
    assert_eq!(sheet_of("Worktrees"), None);
    assert_eq!(sheet_of("Issues"), None);
    assert_eq!(sheet_of("UI"), None);
}

#[test]
fn a_docked_panel_is_never_where_a_setting_lives() {
    assert_eq!(sheet_of("Files"), None);
    assert_eq!(sheet_of("Browser"), None);
}

#[test]
fn a_row_in_view_keeps_the_page_where_it_is() {
    assert!((offset_showing((300., 350.), (100., 700.), -40.) + 40.).abs() < f32::EPSILON);
}

#[test]
fn a_row_below_the_view_comes_up_to_a_third_of_it() {
    assert!((offset_showing((1000., 1050.), (100., 700.), 0.) + 700.).abs() < f32::EPSILON);
}

#[test]
fn a_row_above_the_view_comes_down_to_a_third_of_it() {
    assert!((offset_showing((0., 50.), (100., 700.), -500.) + 200.).abs() < f32::EPSILON);
}

#[test]
fn landing_on_appearance_marks_nothing_and_says_nothing() {
    let found = FoundSetting { title: "Appearance".into(), page: "Appearance".into(), ..FoundSetting::default() };
    assert_eq!(spot(&described(), &found), Spot::default());
}
