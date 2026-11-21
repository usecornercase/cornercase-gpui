use super::*;

#[test]
fn the_queues_key_while_a_bots_routines_show_opens_the_projects_queue() {
    assert!(!closes(Some(PanelKind::Queue), PanelKind::Queue, true));
    assert!(closes(Some(PanelKind::Queue), PanelKind::Queue, false));
}

#[test]
fn another_sheets_key_closes_or_opens_as_before() {
    assert!(closes(Some(PanelKind::Settings), PanelKind::Settings, true));
    assert!(!closes(Some(PanelKind::Queue), PanelKind::Settings, true));
    assert!(!closes(None, PanelKind::Queue, false));
}

#[test]
fn the_routines_sheet_names_its_bot() {
    assert_eq!(routines_title("PRs"), "Routines › PRs");
}
