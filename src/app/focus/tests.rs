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

const ISSUES: PanelKind = PanelKind::Issues;

fn over(dialog: bool, sheet: Option<PanelKind>) -> Overlays {
    Overlays { dialog, sheet, full: false }
}

fn change(before: Overlays, now: Overlays, back: Option<Place>) -> Change {
    Change { before, now, dock: None, in_dock: false, to_terminal: false, back }
}

fn ended_over_issues(back: Option<Place>) -> Change {
    change(over(true, Some(ISSUES)), over(false, Some(ISSUES)), back)
}

#[test]
fn escape_on_a_dialog_over_a_sheet_gives_the_keys_back_to_the_sheet() {
    assert_eq!(keys_after(&ended_over_issues(Some(Place::Sheet(ISSUES)))), Keys::Back);
}

#[test]
fn a_dialog_that_closes_its_sheet_leaves_the_keys_with_the_terminal() {
    let change = change(over(true, Some(ISSUES)), over(false, None), Some(Place::Sheet(ISSUES)));
    assert_eq!(keys_after(&change), Keys::Root);
}

#[test]
fn an_answer_that_asks_for_the_terminal_wins_over_the_way_back() {
    let change = Change { to_terminal: true, ..ended_over_issues(Some(Place::Sheet(ISSUES))) };
    assert_eq!(keys_after(&change), Keys::Root);
}

#[test]
fn a_dialog_over_the_terminal_gives_the_keys_back_to_it() {
    let change = change(over(true, None), over(false, None), Some(Place::Terminal));
    assert_eq!(keys_after(&change), Keys::Back);
}

#[test]
fn a_dialog_over_a_docked_panel_gives_the_keys_back_to_the_panel() {
    let change = Change {
        dock: Some(PanelKind::Changes),
        ..change(over(true, None), over(false, None), Some(Place::Dock(PanelKind::Changes)))
    };
    assert_eq!(keys_after(&change), Keys::Back);
}

#[test]
fn a_docked_panel_closed_meanwhile_does_not_get_the_keys_back() {
    let change = change(over(true, None), over(false, None), Some(Place::Dock(PanelKind::Changes)));
    assert_eq!(keys_after(&change), Keys::Root);
}

#[test]
fn a_sidebar_field_gets_the_keys_back_when_nothing_covers_it() {
    let change = change(over(true, None), over(false, None), Some(Place::Window));
    assert_eq!(keys_after(&change), Keys::Back);
}

#[test]
fn a_sheet_opened_by_the_answer_covers_what_had_the_keys() {
    let change = change(over(true, None), over(false, Some(PanelKind::Settings)), Some(Place::Window));
    assert_eq!(keys_after(&change), Keys::Root);
}

#[test]
fn nothing_to_go_back_to_takes_the_keys_to_the_window_at_once() {
    assert_eq!(keys_after(&ended_over_issues(None)), Keys::Root);
}

#[test]
fn a_dialog_that_gives_way_and_comes_back_keeps_the_keys_where_it_puts_them() {
    let change = change(over(true, None), over(true, None), None);
    assert_eq!(keys_after(&change), Keys::Stay);
}

#[test]
fn in_a_file_s_full_screen_the_dock_card_takes_the_keys() {
    let now = Overlays { full: true, ..over(false, None) };
    let change = change(over(true, None), now, None);
    assert_eq!(keys_after(&change), Keys::DockCard);
}

#[test]
fn a_sheet_that_closes_takes_the_keys_to_the_terminal() {
    assert_eq!(keys_after(&change(over(false, Some(ISSUES)), over(false, None), None)), Keys::Root);
}

#[test]
fn a_docked_panel_that_closes_with_the_keys_gives_them_to_the_terminal() {
    let change = Change { in_dock: true, ..change(over(false, None), over(false, None), None) };
    assert_eq!(keys_after(&change), Keys::Root);
}

#[test]
fn a_sheet_that_opens_leaves_the_keys_to_it() {
    assert_eq!(keys_after(&change(over(false, None), over(false, Some(ISSUES)), None)), Keys::Stay);
}

#[test]
fn what_had_the_keys_is_kept_while_a_dialog_shows() {
    let mut noted = Noted::default();
    noted.note(false, Some((Place::Window, 1)));
    noted.note(true, Some((Place::Terminal, 2)));
    assert_eq!(noted.take(), Some((Place::Window, 1)));
}

#[test]
fn a_dialog_put_in_another_s_place_goes_back_to_what_had_the_keys_before_the_first() {
    let mut noted = Noted::default();
    noted.note(false, Some((Place::Sheet(ISSUES), 1)));
    noted.note(true, Some((Place::Terminal, 2)));
    noted.note(true, Some((Place::Terminal, 3)));
    assert_eq!(noted.take(), Some((Place::Sheet(ISSUES), 1)));
}

#[test]
fn what_was_given_back_is_not_given_twice() {
    let mut noted = Noted::default();
    noted.note(false, Some((Place::Window, 1)));
    noted.take();
    assert_eq!(noted.take(), None);
}
