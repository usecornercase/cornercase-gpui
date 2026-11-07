use super::*;
use crate::model::machines::LinkState;
use crate::test_util::fixture;

const VPS_BOOT: &str = "3085888-1794015402206399212";
const SEEN: &str = "2026-11-06T01:23:55.000Z";

fn session() -> Session {
    serde_json::from_str(fixture!("machines/agents_status.json")).expect("parses")
}

fn rows(session: &Session) -> Vec<SectionRow<'_>> {
    session.section_agents(&session.project_order(), |_| false)
}

fn named(row: &SectionRow<'_>) -> String {
    match row {
        SectionRow::Here(row) => format!("here {}", row.pane.id),
        SectionRow::There(row) => format!("{} {}", row.agent.seen.name, row.agent.pane.id.0),
    }
}

fn walked(session: &Session) -> Vec<(String, usize)> {
    rows(session).iter().map(|row| (named(row), row.depth())).collect()
}

fn there_row<'a>(rows: &'a [SectionRow<'a>], machine: &str, pane: u64) -> &'a ThereRow<'a> {
    rows.iter()
        .find_map(|row| match row {
            SectionRow::There(row) if row.agent.seen.name == machine && row.agent.pane.id == ThereId::heard(pane) => {
                Some(row)
            }
            _ => None,
        })
        .expect("listed")
}

fn there_panes(session: &mut Session) -> impl Iterator<Item = &mut PaneInfo<ThereId>> {
    let projects = session.machines.iter_mut().flat_map(|seen| &mut seen.projects);
    projects.flat_map(|there| &mut there.workspaces).flat_map(|ws| &mut ws.tabs).flat_map(|tab| &mut tab.panes)
}

fn there_pane(session: &mut Session, pane: u64) -> &mut PaneInfo<ThereId> {
    there_panes(session).find(|p| p.id == ThereId::heard(pane)).expect("there")
}

fn folded(pane: u64) -> Session {
    let mut session = session();
    there_pane(&mut session, pane).collapsed = true;
    session
}

fn expected(rows: &[(&str, usize)]) -> Vec<(String, usize)> {
    rows.iter().map(|(name, depth)| ((*name).to_string(), *depth)).collect()
}

fn aim(pane: u64) -> PaneAim {
    PaneAim { machine: "vps".into(), boot: VPS_BOOT.into(), pane: ThereId::heard(pane), collapse: true }
}

#[test]
fn a_projects_own_agents_come_first_then_its_pairs_then_those_with_no_pair() {
    let session = session();
    assert_eq!(
        walked(&session),
        expected(&[
            ("here 11", 0),
            ("here 9", 0),
            ("vps 9", 0),
            ("vps 13", 1),
            ("vps 20", 1),
            ("vps 11", 0),
            ("vps 18", 0),
        ])
    );
}

#[test]
fn a_machines_child_hangs_under_its_parent_there_and_not_under_a_pane_here_with_that_id() {
    let session = session();
    let rows = rows(&session);
    let here = rows.iter().find(|row| named(row) == "here 9").expect("listed");
    assert!(matches!(here, SectionRow::Here(row) if row.children == 0));
    assert_eq!(there_row(&rows, "vps", 9).children, 2);
}

#[test]
fn two_machines_counting_panes_alike_keep_their_trees_apart() {
    let mut session = session();
    let lab = MachineSeen { name: "lab".into(), ..session.machines[0].clone() };
    session.machines.push(lab);
    let rows = rows(&session);
    assert_eq!(there_row(&rows, "vps", 9).children, 2);
    assert_eq!(there_row(&rows, "lab", 9).children, 2);
    assert_eq!(there_row(&rows, "lab", 13).depth, 1);
    let at = |name: &str| rows.iter().position(|row| named(row) == name).expect("listed");
    assert_eq!((at("vps 13"), at("lab 13")), (at("vps 9") + 1, at("lab 9") + 1));
}

#[test]
fn a_machines_project_paired_with_nothing_here_lists_its_agents_after_every_project() {
    let mut session = session();
    session.projects.retain(|project| project.name != "shop");
    assert_eq!(
        walked(&session),
        expected(&[("here 11", 0), ("vps 18", 0), ("vps 9", 0), ("vps 13", 1), ("vps 20", 1), ("vps 11", 0)])
    );
}

#[test]
fn a_shell_on_a_machine_is_no_agent() {
    let session = session();
    assert!(rows(&session).iter().all(|row| !["vps 1", "vps 5", "vps 15"].contains(&named(row).as_str())));
}

#[test]
fn a_machine_with_no_snapshot_adds_no_rows() {
    let mut session = session();
    session.machines[0].projects.clear();
    assert_eq!(walked(&session), expected(&[("here 11", 0), ("here 9", 0)]));
}

#[test]
fn a_workspace_leaving_there_takes_its_agents_with_it_and_its_children_elsewhere_become_roots() {
    let session = session();
    let leaving = |aim: &Aim| aim.workspace == ThereId::heard(7);
    let rows = session.section_agents(&session.project_order(), leaving);
    let walked: Vec<_> = rows.iter().map(|row| (named(row), row.depth())).collect();
    assert_eq!(walked, expected(&[("here 11", 0), ("here 9", 0), ("vps 20", 0), ("vps 18", 0)]));
}

#[test]
fn a_fold_there_hides_every_level_and_says_how_many() {
    let mut session = folded(9);
    there_pane(&mut session, 20).parent = Some(ThereId::heard(13));
    let rows = rows(&session);
    let parent = there_row(&rows, "vps", 9);
    assert_eq!(parent.hidden.len(), 2);
    assert_eq!(parent.children, 1);
    assert_eq!(rows.len(), 5);
}

#[test]
fn a_fold_on_the_machine_leaves_a_pane_here_with_that_id_alone() {
    let mut session = folded(9);
    session.projects.iter_mut().flat_map(|p| &mut p.workspaces).flat_map(|w| &mut w.tabs).for_each(|tab| {
        tab.panes.iter_mut().for_each(|pane| pane.parent = (pane.id == 11).then_some(9));
    });
    let names: Vec<String> = rows(&session).iter().map(named).collect();
    assert!(names.contains(&"here 11".to_string()));
}

#[test]
fn a_folded_row_names_the_loudest_of_what_it_hides() {
    let mut session = folded(9);
    there_pane(&mut session, 13).status = Some("working".into());
    there_pane(&mut session, 20).status = Some("waiting".into());
    let rows = rows(&session);
    assert_eq!(there_row(&rows, "vps", 9).attention(), Some("waiting"));
}

#[test]
fn a_working_agent_behind_a_fold_says_nothing() {
    let mut session = folded(9);
    there_pane(&mut session, 13).status = Some("working".into());
    let rows = rows(&session);
    assert_eq!(there_row(&rows, "vps", 9).attention(), None);
}

#[test]
fn the_aim_is_the_opposite_of_what_the_row_draws() {
    let open = session();
    let rows_open = rows(&open);
    assert!(there_row(&rows_open, "vps", 9).fold_aim().collapse);
    let shut = folded(9);
    let rows_shut = rows(&shut);
    assert!(!there_row(&rows_shut, "vps", 9).fold_aim().collapse);
}

#[test]
fn a_paired_project_reads_by_this_computers_label() {
    let mut session = session();
    let ours = session.projects.iter_mut().find(|project| project.id == 8).expect("shop");
    ours.name = "the shop".into();
    let rows = rows(&session);
    assert_eq!(session.there_place(&there_row(&rows, "vps", 9).agent), "the shop › main");
}

#[test]
fn a_project_with_no_pair_reads_by_the_machines_name_and_one_workspace_says_no_more() {
    let session = session();
    let rows = rows(&session);
    assert_eq!(session.there_place(&there_row(&rows, "vps", 18).agent), "work");
}

#[test]
fn the_shown_tab_lights_its_active_pane_on_that_machine() {
    let session = session();
    let rows = rows(&session);
    let child = there_row(&rows, "vps", 13);
    let shown = child.agent.place();
    assert!(child.lit_by(&shown));
    assert!(!there_row(&rows, "vps", 9).lit_by(&shown));
}

#[test]
fn a_shown_pane_behind_a_fold_lights_the_row_that_hides_it() {
    let session = folded(9);
    let rows = rows(&session);
    let parent = there_row(&rows, "vps", 9);
    let hidden = parent.hidden.iter().find(|agent| agent.pane.id == ThereId::heard(13)).expect("hidden");
    assert!(parent.lit_by(&hidden.place()));
}

#[test]
fn a_tab_with_those_ids_on_another_machine_lights_nothing() {
    let mut session = session();
    let lab = MachineSeen { name: "lab".into(), ..session.machines[0].clone() };
    session.machines.push(lab);
    let rows = rows(&session);
    let shown = there_row(&rows, "lab", 9).agent.place();
    assert!(!there_row(&rows, "vps", 9).lit_by(&shown));
}

#[test]
fn folding_goes_through_while_the_machine_is_linked_in_the_same_run_and_lists_the_pane() {
    let session = session();
    assert!(sighted_pane(&session.machines, &aim(9)).is_ok());
}

#[test]
fn folding_on_a_machine_that_fell_says_its_state() {
    let mut session = session();
    session.machines[0].state = LinkState::Unreachable;
    assert_eq!(sighted_pane(&session.machines, &aim(9)).map(|_| ()), Err("vps unreachable".to_string()));
}

#[test]
fn folding_after_the_machine_restarted_says_so() {
    let mut session = session();
    session.machines[0].boot = "later".into();
    assert_eq!(sighted_pane(&session.machines, &aim(9)).map(|_| ()), Err("vps restarted".to_string()));
}

#[test]
fn folding_a_pane_the_snapshot_no_longer_lists_says_so() {
    let session = session();
    let refused = sighted_pane(&session.machines, &aim(77)).map(|_| ());
    assert_eq!(refused, Err("that agent is no longer on vps".to_string()));
}

#[test]
fn the_link_is_looked_at_before_the_run_and_the_run_before_the_pane() {
    let mut session = session();
    session.machines[0].boot = "later".into();
    let restarted = sighted_pane(&session.machines, &aim(77)).map(|_| ());
    assert_eq!(restarted, Err("vps restarted".to_string()));
    session.machines[0].state = LinkState::Unreachable;
    let down = sighted_pane(&session.machines, &aim(77)).map(|_| ());
    assert_eq!(down, Err("vps unreachable".to_string()));
}

#[test]
fn folding_a_machine_no_longer_listed_says_so() {
    let refused = sighted_pane(&[], &aim(9)).map(|_| ());
    assert_eq!(refused, Err("vps is no longer listed".to_string()));
}

fn fallen(retry_in: Option<i64>, seen: Option<&str>) -> (Session, i64) {
    let mut session = session();
    let now = crate::panels::flows::model::epoch(SEEN).expect("a time") + 120;
    let vps = &mut session.machines[0];
    vps.state = LinkState::Unreachable;
    vps.seen = seen.map(str::to_string);
    vps.retry_at = retry_in.map(|left| format!("2026-11-06T01:25:{:02}.000Z", 55 + left));
    (session, now)
}

fn note(session: &Session, now: i64) -> Option<String> {
    let rows = rows(session);
    let row = there_row(&rows, "vps", 9);
    row.agent.seen.pane_note(row.agent.pane, now)
}

#[test]
fn a_fallen_machines_row_says_the_status_and_when_it_was_seen_and_leaves_the_next_try_to_its_tab() {
    let (session, now) = fallen(Some(4), Some(SEEN));
    assert_eq!(note(&session, now).as_deref(), Some("idle · seen 2 min ago"));
    let rows = rows(&session);
    let row = there_row(&rows, "vps", 9);
    let tab = row.agent.seen.tab_note(row.agent.tab, now);
    assert_eq!(tab.as_deref(), Some("idle · reconnecting in 4 s · seen 2 min ago"));
}

#[test]
fn a_fallen_machines_row_never_seen_says_only_the_status() {
    let (never, now) = fallen(Some(4), None);
    assert_eq!(note(&never, now).as_deref(), Some("idle"));
}

#[test]
fn a_linked_machines_row_has_no_note() {
    assert_eq!(note(&session(), 0), None);
}

#[test]
fn a_row_counts_while_it_says_when_the_machine_was_seen() {
    let (seen, _) = fallen(None, Some(SEEN));
    assert!(seen.machines[0].pane_note_counts());
}

#[test]
fn a_row_with_nothing_that_moves_needs_no_clock() {
    let (never, _) = fallen(Some(4), None);
    assert!(!never.machines[0].pane_note_counts());
    assert!(!session().machines[0].pane_note_counts());
}

#[test]
fn the_fold_names_that_pane_there() {
    assert_eq!(aim(9).args(), ["collapse", "--pane", "9"]);
    assert_eq!(PaneAim { collapse: false, ..aim(9) }.args(), ["expand", "--pane", "9"]);
}

#[test]
fn the_fold_says_what_it_does_and_why_it_failed() {
    assert_eq!(aim(9).running(), "Collapsing the agent on vps…");
    assert_eq!(aim(9).label(), "Collapse");
    assert_eq!(aim(9).failed("no pane 9"), "could not collapse the agent on vps: no pane 9");
    let expand = PaneAim { collapse: false, ..aim(9) };
    assert_eq!(expand.running(), "Expanding the agent on vps…");
    assert_eq!(expand.label(), "Expand");
    assert_eq!(expand.failed("no pane 9"), "could not expand the agent on vps: no pane 9");
}
