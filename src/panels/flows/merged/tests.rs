use super::*;

const LIST: &str = crate::test_util::fixture!("flows/list.json");

fn flow(name: &str) -> FlowInfo {
    let listing = super::super::model::parse_listing(LIST).expect("the list");
    listing.flows.into_iter().find(|f| f.name == name).expect("a flow of that name")
}

fn ends_in_commits(name: &str) -> FlowInfo {
    let mut info = flow("Arena");
    info.name = name.into();
    info.flow.end = "commits".into();
    info
}

#[expect(clippy::unnecessary_wraps, reason = "the releases are optional")]
fn new() -> Option<Version> {
    Some(CLOSE_WHEN_MERGED_SINCE)
}

#[expect(clippy::unnecessary_wraps, reason = "the releases are optional")]
fn old() -> Option<Version> {
    Some(Version::new(0, 13, 21))
}

fn run(closed: bool, commits: bool, close_when_merged: bool) -> Run {
    let mut run = Run { id: 9, workspace: 7, close_when_merged, ..Run::default() };
    run.state = closed.then(|| "closed".to_string());
    run.flow.end = if commits { "commits".into() } else { "pull_request".into() };
    run
}

#[test]
fn a_flow_that_leaves_the_commits_rests_the_choice() {
    let quick = ends_in_commits("Quick");
    let line = rests(Some(&quick), &Releases { here: new(), machine: None }).expect("rests");
    assert_eq!(line, "Quick leaves the commits, so there is no pull request to wait for.");
}

#[test]
fn an_older_server_rests_the_choice_and_names_its_release() {
    let line = rests(Some(&flow("Arena")), &Releases { here: old(), machine: None });
    assert_eq!(line.as_deref(), Some("Needs cornercase 0.13.33 or newer: the server runs 0.13.21."));
}

#[test]
fn a_server_whose_release_is_unknown_rests_the_choice() {
    let line = rests(Some(&flow("Arena")), &Releases { here: None, machine: None });
    assert_eq!(line.as_deref(), Some("Needs cornercase 0.13.33 or newer."));
}

#[test]
fn an_older_machine_rests_the_choice_and_names_the_machine() {
    let releases = Releases { here: new(), machine: Some(("vps", old())) };
    let line = rests(Some(&flow("Arena")), &releases);
    assert_eq!(line.as_deref(), Some("Needs cornercase 0.13.33 or newer on vps: it runs 0.13.21."));
    let unknown = Releases { here: new(), machine: Some(("vps", None)) };
    assert_eq!(rests(Some(&flow("Arena")), &unknown).as_deref(), Some("Needs cornercase 0.13.33 or newer on vps."));
}

#[test]
fn a_machine_start_rests_while_this_server_is_older() {
    let releases = Releases { here: old(), machine: Some(("vps", new())) };
    let line = rests(Some(&flow("Arena")), &releases);
    assert_eq!(line.as_deref(), Some("Needs cornercase 0.13.33 or newer: the server runs 0.13.21."));
}

#[test]
fn a_new_server_offers_the_choice_for_a_flow_that_opens_a_pull_request() {
    let arena = flow("Arena");
    assert_eq!(rests(Some(&arena), &Releases { here: new(), machine: None }), None);
    assert_eq!(rests(Some(&arena), &Releases { here: new(), machine: Some(("vps", new())) }), None);
}

#[test]
fn a_live_run_shows_whether_it_closes_once_merged() {
    assert_eq!(switch(&run(false, false, true), new()), Switch::Offered(true));
    assert_eq!(switch(&run(false, false, false), new()), Switch::Offered(false));
}

#[test]
fn a_closed_run_and_one_that_leaves_the_commits_have_no_switch() {
    assert_eq!(switch(&run(true, false, false), new()), Switch::Hidden);
    assert_eq!(switch(&run(false, true, false), new()), Switch::Hidden);
}

#[test]
fn an_older_server_rests_a_runs_switch() {
    let line = "Needs cornercase 0.13.33 or newer: the server runs 0.13.21.".to_string();
    assert_eq!(switch(&run(false, false, false), old()), Switch::Rests(line));
}

#[test]
fn turning_it_off_sends_off_and_turning_it_on_does_not() {
    assert_eq!(switch_args(7, true), ["flow", "close-when-merged", "--workspace", "7", "--json"]);
    assert_eq!(switch_args(7, false), ["flow", "close-when-merged", "--workspace", "7", "--json", "--off"]);
}

#[test]
fn the_answer_sets_the_run_it_names() {
    let set = parse_set(r#"{"run":9,"project":2,"workspace":7,"close_when_merged":true,"changed":true}"#).expect("set");
    let mut runs = vec![run(false, false, false), Run { id: 3, ..run(false, false, false) }];
    take(&mut runs, &set);
    assert_eq!(runs.iter().map(|r| r.close_when_merged).collect::<Vec<_>>(), [true, false]);
    assert!(!parse_set(r#"{"run":9}"#).expect("set").close_when_merged);
}

#[test]
fn a_start_says_its_agents_close_once_merged_only_when_the_answer_says_so() {
    assert_eq!(closes_clause(true), ", and its agents close once the pull request merges");
    assert_eq!(closes_clause(false), "");
}

#[test]
fn the_line_under_the_box_names_where_a_machines_agents_come_back() {
    assert!(how(None).contains("Reopen agents"));
    let there = how(Some("vps"));
    assert!(there.contains("cornercase on vps can reopen them") && !there.contains("Reopen agents"));
}
