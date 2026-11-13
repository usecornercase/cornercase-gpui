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
fn each_pair_of_boxes_sends_the_whole_choice() {
    let sent = |close, free| switch_args(7, Choice { close, free });
    let base = ["flow", "close-when-merged", "--workspace", "7", "--json"];
    assert_eq!(sent(false, false), [base.as_slice(), &["--off"]].concat());
    assert_eq!(sent(true, false), base);
    assert_eq!(sent(true, true), [base.as_slice(), &["--free-builds"]].concat());
    assert_eq!(sent(false, true), [base.as_slice(), &["--off"]].concat());
}

#[test]
fn a_press_changes_only_its_box_but_the_close_off_turns_both_off() {
    let choice = |close, free| Choice { close, free };
    assert_eq!(choice(true, true).pressed(Press::Close), choice(false, false));
    assert_eq!(choice(true, false).pressed(Press::Close), choice(false, false));
    assert_eq!(choice(false, false).pressed(Press::Close), choice(true, false));
    assert_eq!(choice(true, false).pressed(Press::Free), choice(true, true));
    assert_eq!(choice(true, true).pressed(Press::Free), choice(true, false));
    assert_eq!(choice(false, true).pressed(Press::Close), choice(true, true));
}

#[test]
fn the_free_flag_is_never_sent_below_its_release() {
    let freed = Run { free_builds: true, ..run(false, false, true) };
    let old = Some(Version::new(0, 13, 62));
    let choice = Choice::of(&freed, old);
    assert_eq!(choice, Choice { close: true, free: false });
    assert!(!switch_args(7, choice).contains(&"--free-builds".to_string()));
    assert!(Choice::of(&freed, Some(FREE_BUILDS_SINCE)).free);
}

#[test]
fn the_second_box_is_hidden_wherever_the_first_is_hidden_or_rests() {
    let here = Some(FREE_BUILDS_SINCE);
    assert_eq!(free_switch(&run(true, false, true), here), Switch::Hidden);
    assert_eq!(free_switch(&run(false, true, true), here), Switch::Hidden);
    assert_eq!(free_switch(&run(false, false, true), old()), Switch::Hidden);
}

#[test]
fn a_server_older_than_0_13_64_rests_the_second_box_and_names_its_release() {
    let line = "Needs cornercase 0.13.64 or newer: the server runs 0.13.62.".to_string();
    assert_eq!(free_switch(&run(false, false, true), Some(Version::new(0, 13, 62))), Switch::Rests(line));
    assert_eq!(free_switch(&run(false, false, true), None), Switch::Hidden);
}

#[test]
fn the_second_box_rests_while_the_close_is_off() {
    let here = Some(FREE_BUILDS_SINCE);
    assert_eq!(free_switch(&run(false, false, false), here), Switch::Rests(FREE_HOW.into()));
}

#[test]
fn the_second_box_shows_whether_the_run_frees_its_builds() {
    let here = Some(FREE_BUILDS_SINCE);
    let freed = Run { free_builds: true, ..run(false, false, true) };
    assert_eq!(free_switch(&freed, here), Switch::Offered(true));
    assert_eq!(free_switch(&run(false, false, true), here), Switch::Offered(false));
}

#[test]
fn the_answer_sets_both_choices_of_the_run_it_names() {
    let set = parse_set(crate::test_util::fixture!("flows/close-when-merged-free.json")).expect("set");
    let mut runs = vec![Run { id: 46, ..run(false, false, false) }, Run { id: 3, ..run(false, false, false) }];
    take(&mut runs, &set);
    assert_eq!(
        runs.iter().map(|r| (r.close_when_merged, r.free_builds)).collect::<Vec<_>>(),
        [(true, true), (false, false)]
    );
    let old = parse_set(crate::test_util::fixture!("flows/close-when-merged.json")).expect("set");
    runs[0].id = 9;
    take(&mut runs, &old);
    assert_eq!((runs[0].close_when_merged, runs[0].free_builds), (true, false));
}

#[test]
fn a_start_says_its_builds_are_freed_only_when_the_answer_says_so() {
    assert_eq!(
        closes_clause(true, true),
        ", and its agents close and their build folders are freed once the pull request merges"
    );
    assert_eq!(closes_clause(true, false), ", and its agents close once the pull request merges");
    assert_eq!(closes_clause(false, false), "");
}

#[test]
fn the_close_line_says_nothing_is_removed_only_while_the_builds_stay() {
    assert!(how(None, false).contains("Nothing is removed: Reopen agents"));
    assert!(!how(None, true).contains("Nothing is removed") && how(None, true).contains("Reopen agents"));
    assert!(how(Some("vps"), false).contains("Nothing is removed: cornercase on vps can reopen them"));
    assert!(!how(Some("vps"), true).contains("Nothing is removed") && how(Some("vps"), true).contains("on vps"));
}

#[test]
fn the_dialogs_second_box_follows_the_machine_chosen() {
    let arena = flow("Arena");
    let box_for = |here, machine, close| free_box(Some(&arena), &Releases { here, machine }, close, false);
    let new = Some(FREE_BUILDS_SINCE);
    let vps = Some(("vps", Some(Version::new(0, 13, 57))));
    assert_eq!(
        box_for(new, vps, true),
        Switch::Rests("Needs cornercase 0.13.64 or newer on vps: it runs 0.13.57.".into())
    );
    assert_eq!(
        box_for(Some(Version::new(0, 13, 62)), None, true),
        Switch::Rests("Needs cornercase 0.13.64 or newer: the server runs 0.13.62.".into())
    );
    assert_eq!(box_for(new, None, false), Switch::Rests(FREE_HOW.into()));
    assert_eq!(box_for(new, None, true), Switch::Offered(false));
    let quick = ends_in_commits("Quick");
    assert_eq!(free_box(Some(&quick), &Releases { here: new, machine: None }, true, false), Switch::Hidden);
}

#[test]
fn the_busy_row_says_which_choice_is_being_set() {
    let on = Choice { close: true, free: true };
    let off = Choice { close: true, free: false };
    assert_eq!(doing("Arena", Press::Free, on), "Setting Arena to free its build folders once merged…");
    assert_eq!(doing("Arena", Press::Free, off), "Setting Arena to keep its build folders…");
    assert_eq!(doing("Arena", Press::Close, on), "Setting Arena to close its agents once merged…");
    assert_eq!(
        doing("Arena", Press::Close, Choice { close: false, free: false }),
        "Setting Arena to keep its agents open…"
    );
}
