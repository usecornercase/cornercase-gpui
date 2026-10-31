use super::{Preview, import_args, parse_preview};

fn preview() -> Preview {
    parse_preview(crate::test_util::fixture!("settings/import-preview.json")).expect("the preview parses")
}

#[test]
fn the_preview_counts_every_change() {
    assert_eq!(preview().changes(), 5);
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
    assert!(preview().risky());
    let safe = parse_preview(
        r#"{"sections":[{"id":"issues","changes":[{"label":"a","from":"b","to":"c"}],
        "notes":[{"kind":"repository","text":"open it"}]}],"notes":[]}"#,
    )
    .expect("parses");
    assert!(!safe.risky());
}

#[test]
fn sections_without_changes_or_notes_are_left_out() {
    let shown: Vec<String> = preview().shown().map(|s| s.id.clone()).collect();
    assert_eq!(shown, ["agents", "issues", "repositories"]);
}

#[test]
fn the_setup_goes_on_standard_input() {
    assert_eq!(import_args(true), ["config", "import", "-", "--preview", "--json"]);
    assert_eq!(import_args(false), ["config", "import", "-", "--yes", "--json"]);
}
