use super::shown_run;
use crate::panels::flows::model::parse_runs;

const RUNS: &str = crate::test_util::fixture!("flows/runs.json");

#[test]
fn the_dock_shows_the_run_of_the_shown_workspace() {
    let runs = parse_runs(RUNS).expect("runs").runs;
    assert_eq!(shown_run(&runs, Some(3)), Some(7));
}

#[test]
fn a_workspace_without_a_run_shows_none() {
    let runs = parse_runs(RUNS).expect("runs").runs;
    assert_eq!(shown_run(&runs, Some(4)), None);
    assert_eq!(shown_run(&runs, None), None);
    assert_eq!(shown_run(&[], Some(3)), None);
}
