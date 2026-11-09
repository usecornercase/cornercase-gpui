use super::*;
use crate::test_util::fixture;

fn sound() -> Vec<FoundSetting> {
    parse_found(fixture!("settings/search/sound.json")).expect("the answer parses")
}

fn machines() -> Vec<FoundSetting> {
    parse_found(fixture!("settings/search/machines.json")).expect("the answer parses")
}

fn answered(query: &str, hits: Vec<FoundSetting>) -> Asked {
    Asked { answers: query.into(), hits, ..Asked::default() }
}

#[test]
fn the_answer_reads_what_lives_where_and_its_value() {
    let hits = sound();
    assert_eq!(hits.len(), 4);
    assert_eq!(hits[0].title, "UI › sound");
    assert_eq!(hits[0].shown, "[x] on");
    assert_eq!(hits[0].key.as_deref(), Some("sound"));
    assert!(!hits[0].hidden);
}

#[test]
fn a_row_not_shown_yet_reads_as_hidden_with_what_it_needs() {
    let hits = parse_found(fixture!("settings/search/forum.json")).expect("the answer parses");
    assert!(hits.iter().all(|hit| hit.hidden));
    assert_eq!(note_of(&hits[0]), "shows once a phone is paired");
}

#[test]
fn the_machines_tab_lists_its_entries_without_keys() {
    let hits = machines();
    let titles: Vec<&str> = hits.iter().map(|hit| hit.title.as_str()).collect();
    assert_eq!(
        titles,
        ["Machines", "Machines › ▭ this computer", "Machines › ☁ vps", "Machines › ▣ down", "Machines › + add machine"]
    );
    assert!(hits.iter().all(|hit| hit.key.is_none()));
}

#[test]
fn a_tab_says_it_is_a_page_or_a_sheet() {
    let tab = |name: &str| FoundSetting { title: name.into(), page: name.into(), ..FoundSetting::default() };
    assert_eq!(note_of(&tab("UI")), "the page");
    assert_eq!(note_of(&tab("Accounts")), "the sheet");
    assert_eq!(note_of(&tab("Machines")), "the sheet");
}

#[test]
fn a_note_about_the_tuis_keys_is_dropped() {
    let hits = parse_found(fixture!("settings/search/accounts.json")).expect("the answer parses");
    let token = hits.iter().find(|hit| hit.key.as_deref() == Some("linear")).expect("the tracker");
    assert_eq!(token.note, "enter pastes one");
    assert_eq!(note_of(token), "");
}

#[test]
fn it_asks_with_the_query_after_the_separator() {
    assert_eq!(ask_args("--sound"), ["config", "search", "--json", "--", "--sound"]);
}

#[test]
fn the_status_adds_the_settings_after_the_names() {
    let asked = answered("sound", sound());
    assert_eq!(asked.status("3 results".into(), "sound", 3), "3 results · 4 settings");
}

#[test]
fn with_only_settings_the_status_counts_them() {
    let asked = answered("sound", sound());
    assert_eq!(asked.status("Nothing matches “sound”".into(), "sound", 0), "4 settings");
    let one = answered("sound", vec![FoundSetting::default()]);
    assert_eq!(one.status("Nothing matches “sound”".into(), "sound", 0), "1 setting");
}

#[test]
fn nothing_matches_only_once_the_settings_answered() {
    let waiting = answered("sou", sound());
    assert_eq!(waiting.status("Nothing matches “zz”".into(), "zz", 0), "Looking in the settings…");
    let empty = answered("zz", Vec::new());
    assert_eq!(empty.status("Nothing matches “zz”".into(), "zz", 0), "Nothing matches “zz”");
}

#[test]
fn names_keep_their_status_while_the_settings_are_asked() {
    let waiting = answered("sou", sound());
    assert_eq!(waiting.status("2 results".into(), "soun", 2), "2 results");
}

#[test]
fn an_empty_query_waits_for_nothing() {
    let asked = Asked::default();
    assert!(!asked.waiting_for(""));
    assert_eq!(asked.status("Everything open".into(), "", 3), "Everything open");
}

#[test]
fn a_failed_answer_says_why_and_stops_waiting() {
    let failed = Asked { error: Some("the server is gone".into()), ..Asked::default() };
    assert!(failed.failed() && !failed.waiting_for("sound"));
    assert_eq!(failed.status("3 results".into(), "sound", 3), "3 results · the server is gone");
}

#[test]
fn hits_of_another_query_are_stale_until_the_answer() {
    let asked = answered("sou", sound());
    assert!(asked.waiting_for("soun"));
    assert!(!asked.waiting_for("sou"));
}

fn appearance_row() -> FoundSetting {
    FoundSetting { title: "Appearance".into(), page: "Appearance".into(), ..FoundSetting::default() }
}

#[test]
fn appearance_is_listed_for_its_words_before_cornercase_s_settings() {
    let asked = Asked { own: vec![appearance_row()], hits: sound(), ..Asked::default() };
    assert_eq!(asked.hits().len(), 5);
    assert_eq!(asked.hits()[0], appearance_row());
}

#[test]
fn appearance_stays_listed_when_config_search_fails() {
    let failed = Asked { own: vec![appearance_row()], error: Some("the server is gone".into()), ..Asked::default() };
    assert_eq!(failed.hits().len(), 1);
    assert!(failed.failed());
}
