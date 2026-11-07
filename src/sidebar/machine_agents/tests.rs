use super::*;
use crate::model::machines::LinkState;
use crate::model::{SectionRow, Session};
use crate::panels::flows::model::epoch;
use crate::test_util::fixture;

const SEEN: &str = "2026-11-06T01:23:55.000Z";

fn session() -> Session {
    serde_json::from_str(fixture!("machines/agents_status.json")).expect("parses")
}

fn first_there<'a>(rows: &'a [SectionRow<'a>]) -> &'a ThereRow<'a> {
    rows.iter()
        .find_map(|row| match row {
            SectionRow::There(row) => Some(row),
            SectionRow::Here(_) => None,
        })
        .expect("a row on the machine")
}

fn line(session: &Session, now: i64) -> String {
    let rows = session.section_agents(&session.project_order(), |_| false);
    let row = first_there(&rows);
    there_line(row, Some(session.there_place(&row.agent)), &Look::default(), now)
}

fn fell(session: &mut Session) -> i64 {
    session.machines[0].state = LinkState::Unreachable;
    session.machines[0].seen = Some(SEEN.into());
    epoch(SEEN).expect("a time") + 120
}

#[test]
fn a_row_names_its_place_then_what_the_agent_is() {
    assert_eq!(line(&session(), 0), "shop › main · claude");
}

#[test]
fn a_machine_that_fell_says_when_it_was_last_seen_instead_of_the_details() {
    let mut session = session();
    let now = fell(&mut session);
    assert_eq!(line(&session, now), "shop › main · idle · seen 2 min ago");
}

#[test]
fn a_machine_that_fell_says_the_status_it_last_heard() {
    let mut session = session();
    let now = fell(&mut session);
    let panes = session.machines[0].projects.iter_mut().flat_map(|p| &mut p.workspaces).flat_map(|w| &mut w.tabs);
    panes.flat_map(|tab| &mut tab.panes).for_each(|pane| pane.status = Some("waiting".into()));
    assert_eq!(line(&session, now), "shop › main · waiting · seen 2 min ago");
}

#[test]
fn the_row_is_another_element_once_the_machine_restarted() {
    let mut session = session();
    let before = {
        let rows = session.section_agents(&session.project_order(), |_| false);
        row_id(first_there(&rows))
    };
    session.machines[0].boot = "later".into();
    let rows = session.section_agents(&session.project_order(), |_| false);
    assert_ne!(before, row_id(first_there(&rows)));
}

#[test]
fn a_wide_row_keeps_all_three_tags() {
    let fit = fit_tags(234., true, 3, "vps");
    assert_eq!(fit, Fit { attention: true, hidden: true, machine: true });
}

#[test]
fn a_row_with_nothing_folded_has_only_the_machine() {
    let fit = fit_tags(234., false, 0, "vps");
    assert_eq!(fit, Fit { attention: false, hidden: false, machine: true });
}

#[test]
fn a_narrower_row_drops_the_count_first() {
    let fit = fit_tags(116., true, 3, "vps");
    assert_eq!(fit, Fit { attention: true, hidden: false, machine: true });
}

#[test]
fn the_count_never_outlasts_the_machine_though_it_is_smaller() {
    let fit = fit_tags(112., true, 3, "vps");
    assert_eq!(fit, Fit { attention: true, hidden: false, machine: false });
}

#[test]
fn narrower_still_it_drops_the_machine_and_the_waiting_agent_stays() {
    let fit = fit_tags(90., true, 3, "vps");
    assert_eq!(fit, Fit { attention: true, hidden: false, machine: false });
}

#[test]
fn a_row_with_no_room_says_nothing_rather_than_running_over_the_place() {
    let fit = fit_tags(66., true, 3, "vps");
    assert!(!fit.any());
}

#[test]
fn a_long_machine_name_needs_no_more_room_than_a_short_one_since_it_is_cut() {
    assert_eq!(fit_tags(234., true, 3, "a-very-long-machine-name"), fit_tags(234., true, 3, "lab"));
}
