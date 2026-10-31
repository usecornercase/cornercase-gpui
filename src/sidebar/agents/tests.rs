use super::*;
use crate::model::PaneInfo;

#[test]
fn a_child_names_its_place_only_when_it_works_elsewhere() {
    let pane = PaneInfo::default();
    let row = |workspace: u64, depth: usize| AgentRow { pane: &pane, workspace, tab: 0, depth, children: 0 };
    let rows = [row(1, 0), row(1, 1), row(2, 1), row(2, 2), row(1, 0), row(3, 0)];
    assert_eq!(places_shown(&rows), [true, false, true, false, true, true]);
}
