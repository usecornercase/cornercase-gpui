use std::collections::HashSet;

use super::{Preview, import_args, parse_preview};

fn preview() -> Preview {
    parse_preview(crate::test_util::fixture!("settings/import-preview.json")).expect("the preview parses")
}

fn unticked(sections: &[&str]) -> HashSet<String> {
    sections.iter().map(ToString::to_string).collect()
}

#[test]
fn the_preview_counts_every_change() {
    assert_eq!(preview().changes(&unticked(&[])), 5);
}

#[test]
fn an_unticked_section_does_not_count() {
    assert_eq!(preview().changes(&unticked(&["agents"])), 1);
    assert_eq!(preview().changes(&unticked(&["agents", "issues"])), 0);
}

#[test]
fn with_every_section_ticked_the_import_names_none() {
    assert_eq!(preview().only(&unticked(&[])), None);
}

#[test]
fn an_unticked_section_leaves_the_others_named() {
    let only = preview().only(&unticked(&["agents"]));
    assert_eq!(only.as_deref(), Some("issues,worktrees,interface,repositories"));
}

#[test]
fn unticking_the_risky_section_takes_its_risk_away() {
    assert!(!preview().risky(&unticked(&["agents"])));
}

#[test]
fn the_preview_reads_each_change_as_from_and_to() {
    let preview = preview();
    let change = &preview.sections[1].changes[0];
    assert_eq!(
        (change.label.as_str(), change.from.as_str(), change.to.as_str()),
        ("Jira site", "not set", "acme.atlassian.net")
    );
}

#[test]
fn a_dangerous_setting_makes_the_import_risky() {
    assert!(preview().risky(&unticked(&[])));
    let safe = parse_preview(
        r#"{"sections":[{"id":"issues","changes":[{"label":"a","from":"b","to":"c"}],
        "notes":[{"kind":"repository","text":"open it"}]}],"notes":[]}"#,
    )
    .expect("parses");
    assert!(!safe.risky(&unticked(&[])));
}

#[test]
fn sections_without_changes_or_notes_are_left_out() {
    let shown: Vec<String> = preview().shown().map(|s| s.id.clone()).collect();
    assert_eq!(shown, ["agents", "issues", "repositories"]);
}

#[test]
fn the_setup_goes_on_standard_input() {
    assert_eq!(import_args(true, None), ["config", "import", "-", "--preview", "--json"]);
    assert_eq!(import_args(false, None), ["config", "import", "-", "--yes", "--json"]);
}

#[test]
fn chosen_sections_go_in_one_option() {
    assert_eq!(
        import_args(false, Some("agents,issues")),
        ["config", "import", "-", "--yes", "--json", "--only=agents,issues"]
    );
}
