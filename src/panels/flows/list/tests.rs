use super::*;
use crate::panels::flows::LIST;

#[test]
fn a_wide_sheet_keeps_the_list_at_its_width_beside_the_page() {
    assert_eq!(columns(1_050., &LIST), Columns::Two { list: 330. });
}

#[test]
fn a_narrower_sheet_narrows_the_list_to_keep_the_page() {
    assert_eq!(columns(902., &LIST), Columns::Two { list: 282. });
    assert_eq!(columns(860., &LIST), Columns::Two { list: 240. });
}

#[test]
fn a_sheet_too_narrow_for_both_shows_one_column() {
    assert_eq!(columns(859., &LIST), Columns::One);
    assert_eq!(columns(525., &LIST), Columns::One);
}

#[test]
fn a_dragged_list_still_leaves_the_page_its_least_width() {
    let dragged = LIST.restored(Some(600.), 2_000.);
    assert_eq!(columns(1_050., &dragged), Columns::Two { list: 430. });
}

#[test]
fn a_rows_state_reads_closed_ended_stopped_moving_or_running() {
    let words = [Standing::Closed, Standing::Ended, Standing::Stopped(90), Standing::Running].map(row_word);
    assert_eq!(words, ["closed", "ended", "stopped moving", "running"]);
}

#[test]
fn only_an_ended_run_names_in_its_row_the_stages_it_ended_without() {
    let owed = Run { owed: vec![2], ..Run::default() };
    assert_eq!(ended_without(&owed), None);
    let ended = Run { ended: true, ..owed };
    assert_eq!(ended_without(&ended).as_deref(), Some("ended with stage 3 not run on the last change"));
    assert_eq!(ended_without(&Run { owed: vec![], ..ended }), None);
}
