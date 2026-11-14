use super::{LIST, Panes, SHEET_AT_1280, Wants};

#[test]
fn an_issues_sheet_of_720_keeps_the_list_at_400_beside_the_issue() {
    assert_eq!(Panes::of(720., &LIST, true, Wants::Issue), Panes::Both { list: 400. });
}

#[test]
fn a_narrower_issues_sheet_narrows_the_list_to_keep_the_issue_320() {
    assert_eq!(Panes::of(654., &LIST, true, Wants::Issue), Panes::Both { list: 334. });
}

#[test]
fn the_issues_list_reaches_its_least_at_a_sheet_of_580() {
    assert_eq!(Panes::of(580., &LIST, true, Wants::Issue), Panes::Both { list: 260. });
}

#[test]
fn under_580_the_chosen_issue_shows_alone() {
    assert_eq!(Panes::of(579., &LIST, true, Wants::Issue), Panes::Issue);
}

#[test]
fn under_580_the_list_shows_alone_when_nothing_is_chosen() {
    assert_eq!(Panes::of(523., &LIST, false, Wants::Issue), Panes::List);
}

#[test]
fn under_580_the_list_shows_alone_once_it_is_asked_for() {
    assert_eq!(Panes::of(523., &LIST, true, Wants::List), Panes::List);
}

#[test]
fn two_columns_show_both_whatever_was_last_asked() {
    assert_eq!(Panes::of(720., &LIST, true, Wants::List), Panes::Both { list: 400. });
}

#[test]
fn a_list_dragged_wide_gives_way_and_comes_back() {
    let dragged = LIST.restored(Some(700.), 2_000.);
    assert_eq!(Panes::of(654., &dragged, true, Wants::Issue), Panes::Both { list: 334. });
    assert_eq!(Panes::of(1_047.5, &dragged, true, Wants::Issue), Panes::Both { list: 700. });
}

#[test]
fn an_issues_sheet_not_yet_measured_draws_two_columns_at_400() {
    assert_eq!(Panes::of(SHEET_AT_1280, &LIST, false, Wants::Issue), Panes::Both { list: 400. });
}

#[test]
fn an_issues_sheet_of_no_width_shows_the_list() {
    assert_eq!(Panes::of(0., &LIST, false, Wants::Issue), Panes::List);
}

#[test]
fn the_rows_are_measured_at_the_width_they_are_drawn() {
    assert!((Panes::Both { list: 334. }.list_width(654.) - 334.).abs() < f32::EPSILON);
    assert!((Panes::List.list_width(523.) - 523.).abs() < f32::EPSILON);
    assert!((Panes::Issue.list_width(523.) - 523.).abs() < f32::EPSILON);
}
