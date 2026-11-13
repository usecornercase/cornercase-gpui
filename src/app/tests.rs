use super::*;

#[test]
fn a_sheet_giving_way_to_another_takes_the_keys_back() {
    assert!(sheet_gave_way(Some(PanelKind::Search), Some(PanelKind::Accounts)));
}

#[test]
fn a_sheet_that_stays_or_opens_or_closes_does_not_give_way() {
    assert!(!sheet_gave_way(Some(PanelKind::Search), Some(PanelKind::Search)));
    assert!(!sheet_gave_way(None, Some(PanelKind::Search)));
    assert!(!sheet_gave_way(Some(PanelKind::Search), None));
    assert!(!sheet_gave_way(None, None));
}

#[test]
fn the_browser_and_the_flow_panel_keep_widths_of_their_own() {
    assert!(DOCKS.wide(PanelKind::Browser).is_some());
    assert!(DOCKS.wide(PanelKind::Flow).is_some());
    assert!(DOCKS.wide(PanelKind::Changes).is_none());
    assert!(DOCKS.wide(PanelKind::Todo).is_none());
}

#[test]
fn the_start_asks_for_its_answer_as_json() {
    let args = start_args((160, 48));
    assert_eq!(args[..2], ["start-server", "--cols"]);
    assert_eq!(args[args.len() - 3..], ["--theme", "-", "--json"]);
}

#[test]
fn a_missing_cornercase_starts_no_server() {
    let missing = cli::Missing::NotFound { places: Vec::new(), asked_shell: false };
    let msg = start_message(Err(missing), || panic!("the server must not start"));
    assert!(matches!(msg, Msg::Missing(_)));
}

#[test]
fn a_found_cornercase_starts_the_server() {
    let msg = start_message(Ok(()), || cli::Start::Up(cli::Started::default()));
    assert!(matches!(msg, Msg::Started(cli::Start::Up(_))));
}
