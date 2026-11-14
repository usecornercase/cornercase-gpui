use super::*;

const RUNS: &str = crate::test_util::fixture!("flows/runs.json");
const CLOSED_RUN: &str = r#"{"revision":1,"runs":[{"run":2,"workspace":5,"flow":{"name":"Q","stages":[{"roles":["c"],"back":null}]},"roles":[],"stage":null,"rounds":[0],"state":"closed"}]}"#;
const OWED: &str = crate::test_util::fixture!("flows/runs-not-run-stopped.json");
const STARTED: &str = crate::test_util::fixture!("flows/started.json");
const STARTED_FREE: &str = crate::test_util::fixture!("flows/started-free.json");
const LIST: &str = crate::test_util::fixture!("flows/list.json");

#[test]
fn reads_a_run_snapshot() {
    let runs = parse_runs(RUNS).expect("runs");
    let run = &runs.runs[0];
    assert_eq!((runs.revision, run.id, run.workspace, run.stage), (3, 7, 3, Some(0)));
    assert_eq!(run.flow.stages[1].back.as_ref().map(|b| b.to), Some(1));
    assert_eq!(run.orchestrator().map(|m| m.pane), Some(Some(10)));
    assert_eq!(run.member("coder").and_then(|m| m.branch.as_deref()), Some("fix--coder"));
    let reviewer = run.member("reviewer").expect("reviewer");
    assert_eq!((reviewer.closed, reviewer.pane, reviewer.tab), (true, Some(12), None));
    assert_eq!(
        (run.last.as_ref().and_then(|s| s.edge), run.outside),
        (Some(Edge::Loop { stage: 1 }), Some(Edge::Outside { from: Some(0), to: 1 }))
    );
    assert_eq!((run.ended, run.closed()), (true, false));
    assert_eq!((run.summary().as_str(), run.loudest()), ("stage 1 of 2 · findings 1/2", Some("pending")));
    assert_eq!(
        (run.flow.role("orchestrator").map(|r| r.model.as_deref()), run.flow.colour_index("reviewer")),
        (Some(Some("opus")), Some(1))
    );
}

#[test]
fn reads_a_closed_run_and_an_empty_one() {
    let run = &parse_runs(CLOSED_RUN).expect("runs").runs[0];
    assert!(run.closed());
    assert_eq!(run.summary(), "not started");
    assert_eq!(parse_runs(r#"{"revision":1,"runs":[],"boot":"x","version":"0.12"}"#).expect("empty").runs.len(), 0);
}

#[test]
fn reads_the_flow_list() {
    let list = parse_listing(LIST).expect("list");
    let mine = &list.flows[1];
    assert_eq!((list.builtins.len(), mine.origin.as_str(), mine.edited, mine.default), (2, "user", true, true));
    assert_eq!(
        (mine.tags(), mine.path.as_deref(), mine.agents()),
        (vec!["default", "edited"], Some("/c/flows/quick.toml"), 3)
    );
    assert_eq!(list.flows[0].tags(), Vec::<&str>::new());
    let project = &list.flows[2];
    assert_eq!(
        (project.can_start(), project.tags(), project.check.as_deref()),
        (false, vec!["this project"], Some("it sets X for coder"))
    );
    assert_eq!((list.hidden[0].name.as_str(), list.broken[0].error.as_str()), ("Arena", "line 3: expected `=`"));
    assert_eq!(
        (list.flows[0].flow.end_label(), list.flows[1].flow.end_label()),
        ("open a pull request", "leave the commits")
    );
    assert_ne!(list.flows[1].key(), list.flows[2].key());
}

#[test]
fn reads_message_frames() {
    let line = r#"{"run":7,"workspace":3,"seq":4,"time":"2026-10-18T09:24:00.000Z","from":"orchestrator","to":"coder","kind":"send","text":"Fix the findings.","bytes":17,"edge":{"kind":"loop","stage":1}}"#;
    let Frame::Message(m) = parse_frame(line).expect("message") else { panic!("not a message") };
    assert_eq!((m.seq, m.verb(), m.edge), (4, "sent", Some(Edge::Loop { stage: 1 })));
    let read = r#"{"run":7,"workspace":3,"seq":3,"time":"t","from":"orchestrator","to":"reviewer","kind":"read","text":null,"bytes":0,"edge":null}"#;
    let Frame::Message(m) = parse_frame(read).expect("read") else { panic!("not a message") };
    assert_eq!((m.text.as_deref(), m.edge, m.verb()), (None, None, "read"));
    assert_eq!(parse_frame(r#"{"event":"dropped","count":5}"#).expect("dropped"), Frame::Dropped(5));
    assert!(parse_frame("not json").is_err());
}

#[test]
fn a_started_run_is_shown_on_its_orchestrator_tab() {
    let started: Started = serde_json::from_str(crate::test_util::fixture!("flows/started.json")).expect("started");
    assert_eq!((started.run, started.workspace, started.branch.as_str()), (11, 10, "check-where-the-view-goes"));
    assert_eq!(started.place(), ("tab", 15));
}

#[test]
fn a_started_run_without_members_is_shown_by_its_workspace() {
    let started: Started = serde_json::from_str(r#"{"run":2,"workspace":5,"branch":"b"}"#).expect("started");
    assert_eq!(started.place(), ("workspace", 5));
}

#[test]
fn reads_run_files() {
    let text = r#"{"workspace":3,"run":7,"dir":"/r","files":[{"name":"review.md","path":"/r/review.md","bytes":2048,"modified":"2026-10-18T09:24:00.000Z"}],"more":false}"#;
    let files = parse_files(text).expect("files");
    assert_eq!((files.dir.as_str(), files.entries[0].bytes), ("/r", 2048));
}

#[test]
fn makes_names_unique_ignoring_case() {
    assert_eq!(unique_name("Mine", &["Quick"]), "Mine");
    assert_eq!(unique_name("Quick", &["quick ", "Quick 2"]), "Quick 3");
    assert_eq!(unique_name(" Arena", &["arena"]), "Arena 2");
}

#[test]
fn moves_a_flow_in_the_order() {
    let names: Vec<String> = ["A", "B", "b", "C"].iter().map(ToString::to_string).collect();
    assert_eq!(moved(&names, 0, 1), Some(vec!["B".into(), "A".into(), "C".into()]));
    assert_eq!(moved(&names, 3, -1), Some(vec!["A".into(), "C".into(), "B".into()]));
    assert_eq!(moved(&names, 2, -1), Some(vec!["B".into(), "A".into(), "C".into()]));
    assert_eq!(moved(&names, 0, -1), None);
    assert_eq!(moved(&names, 3, 1), None);
    assert_eq!(moved(&names, 9, 1), None);
}

#[test]
fn reads_times() {
    assert_eq!(epoch("1970-01-01T00:00:00.000Z"), Some(0));
    assert_eq!(epoch("2026-10-18T09:21:00.000Z"), Some(1_792_315_260));
    assert_eq!(epoch("2000-03-01T00:00:01Z"), Some(951_868_801));
    assert_eq!(epoch("garbage"), None);
    let t = "2026-10-18T09:21:00.000Z";
    let at = epoch(t).unwrap_or_default();
    let ages = [(2, "now"), (30, "30s"), (600, "10m"), (7200, "2h"), (3 * 86_400, "3d")];
    assert_eq!(ages.map(|(after, _)| ago(t, at + after)), ages.map(|(_, age)| age.to_string()));
    assert_eq!(ago("bad", at), "");
    assert_eq!((size(512), size(3400)), ("512 B".to_string(), "3.4 KB".to_string()));
}

#[test]
fn a_role_shows_the_initials_of_its_first_two_words() {
    assert_eq!(initials("coder A"), "CA");
    assert_eq!(initials("second  reviewer here"), "SR");
    assert_eq!(initials("judge"), "J");
    assert_eq!(initials("  "), "?");
}

#[test]
fn a_plan_is_outlined_by_its_stages_and_loops() {
    let runs = parse_runs(RUNS).expect("runs");
    let flow = &runs.runs[0].flow;
    assert_eq!((flow.loops(), flow.outline().as_str()), (1, "2 stages · 1 loop"));
    let single = FlowSpec { stages: vec![StageSpec::default()], ..FlowSpec::default() };
    assert_eq!(single.outline(), "1 stage");
    assert!(!flow.ends_in_commits());
}

#[test]
fn a_run_passes_the_stages_before_the_active_one() {
    let mut run = parse_runs(RUNS).expect("runs").runs.remove(0);
    run.ended = false;
    run.stage = Some(1);
    assert_eq!(
        (run.progress(0), run.progress(1), run.progress(2)),
        (Progress::Passed, Progress::Active, Progress::Ahead)
    );
    run.stage = None;
    assert_eq!(run.progress(0), Progress::Ahead);
    run.ended = true;
    assert_eq!(run.progress(1), Progress::Passed);
}

#[test]
fn a_loop_past_its_rounds_is_over_its_limit() {
    let mut run = parse_runs(RUNS).expect("runs").runs.remove(0);
    assert!(!run.over_limit(1));
    run.rounds = vec![0, 3];
    assert!(run.over_limit(1) && !run.over_limit(0) && !run.over_limit(5));
}

#[test]
fn the_orchestrator_is_known_by_its_name_or_its_role() {
    let runs = parse_runs(RUNS).expect("runs");
    let flow = &runs.runs[0].flow;
    assert!(flow.is_lead("orchestrator"));
    assert!(!flow.is_lead("coder"));
    let renamed = FlowSpec { orchestrator: RoleSpec { name: "lead".into(), ..RoleSpec::default() }, ..flow.clone() };
    assert!(renamed.is_lead("lead") && renamed.is_lead("orchestrator"));
}

#[test]
fn a_loop_label_says_its_limit_or_its_rounds_long_or_short() {
    let back = BackSpec { to: 1, when: "findings".into(), at_most: 3 };
    assert_eq!(back.label(None, false), "findings · at most 3");
    assert_eq!(back.label(None, true), "findings ≤ 3");
    assert_eq!(back.label(Some(1), false), "findings · 1/3");
    assert_eq!(back.label(Some(4), true), "findings 4/3");
}

#[test]
fn a_loop_count_says_its_limit_or_its_rounds_long_or_short() {
    let back = BackSpec { to: 1, when: "findings".into(), at_most: 3 };
    assert_eq!(back.count(None, false), "· at most 3");
    assert_eq!(back.count(None, true), "≤ 3");
    assert_eq!(back.count(Some(1), false), "· 1/3");
    assert_eq!(back.count(Some(4), true), "4/3");
}

#[test]
fn a_run_lists_the_loops_that_went_round_with_their_rounds() {
    let run = &parse_runs(RUNS).expect("runs").runs[0];
    let looped: Vec<_> =
        run.looped().into_iter().map(|(stage, back, rounds)| (stage, back.when.as_str(), rounds)).collect();
    assert_eq!(looped, [(1, "findings", 1)]);
    let closed = &parse_runs(CLOSED_RUN).expect("runs").runs[0];
    assert_eq!(closed.looped(), []);
}

#[test]
fn a_team_names_each_of_its_agents_once_the_orchestrator_first() {
    let listing = parse_listing(LIST).expect("a listing");
    let quick = listing.flows.iter().find(|f| f.origin == "user").expect("the user's flow");
    assert_eq!(quick.team_agents(), ["claude", "codex"]);
}

#[test]
fn a_run_reads_whether_it_closes_once_merged() {
    assert!(!parse_runs(RUNS).expect("runs").runs[0].close_when_merged);
    let line = r#"{"revision":1,"runs":[{"run":2,"workspace":5,"close_when_merged":true}]}"#;
    assert!(parse_runs(line).expect("runs").runs[0].close_when_merged);
}

#[test]
fn a_start_answer_reads_close_when_merged() {
    let started: Started =
        serde_json::from_str(r#"{"run":1,"workspace":2,"branch":"b","close_when_merged":true}"#).expect("an answer");
    assert!(started.close_when_merged);
    let plain: Started = serde_json::from_str(r#"{"run":1,"workspace":2,"branch":"b"}"#).expect("an answer");
    assert!(!plain.close_when_merged);
}

fn owed_run() -> Run {
    parse_runs(OWED).expect("runs").runs.remove(0)
}

fn quiet_for(run: &Run, secs: i64) -> Standing {
    let at = epoch(run.stopped.as_deref().expect("a stop time")).expect("a time");
    run.standing_at(at + secs)
}

#[test]
fn a_run_reads_the_stages_not_run_when_it_stopped_and_whether_it_frees_its_builds() {
    let run = owed_run();
    assert_eq!(run.owed, [1, 2]);
    assert_eq!(run.stopped.as_deref(), Some("2026-11-13T10:00:00.000Z"));
    assert!(run.free_builds && run.close_when_merged);
}

#[test]
fn an_older_servers_run_has_no_stages_owed_no_stop_and_no_free() {
    let runs = parse_runs(RUNS).expect("runs");
    let run = &runs.runs[0];
    assert_eq!((run.owed.is_empty(), run.stopped.as_deref(), run.free_builds), (true, None, false));
    assert_eq!((run.not_run_line(), run.standing_at(i64::MAX), run.owes(0)), (None, Standing::Ended, false));
}

#[test]
fn stages_read_as_words_counted_from_one() {
    assert_eq!(stages_in_words(&[2]), "stage 3");
    assert_eq!(stages_in_words(&[2, 3]), "stage 3 and 4");
    assert_eq!(stages_in_words(&[1, 2, 3]), "stage 2, 3 and 4");
    assert_eq!(stages_in_words(&[]), "");
}

#[test]
fn a_live_run_lists_the_stages_it_has_not_run_since_the_last_change() {
    let run = owed_run();
    assert_eq!(run.not_run_line().as_deref(), Some("stage 2 and 3 not run since the last change"));
    assert_eq!(Run { owed: vec![], ..run }.not_run_line(), None);
}

#[test]
fn an_ended_run_says_which_stages_it_ended_without() {
    let run = Run { ended: true, owed: vec![2], ..owed_run() };
    assert_eq!(run.not_run_line().as_deref(), Some("ended with stage 3 not run on the last change"));
}

#[test]
fn a_run_reads_as_stopped_once_it_has_not_moved_for_a_minute() {
    let run = owed_run();
    assert_eq!(quiet_for(&run, STOPPED_AFTER - 1), Standing::Running);
    assert_eq!(quiet_for(&run, STOPPED_AFTER), Standing::Stopped(STOPPED_AFTER));
    assert_eq!(quiet_for(&run, 600), Standing::Stopped(600));
    assert_eq!(Run { stopped: None, ..run }.standing_at(i64::MAX), Standing::Running);
}

#[test]
fn a_closed_or_ended_run_is_never_stopped() {
    let run = owed_run();
    let closed = Run { state: Some("closed".into()), ..run.clone() };
    assert_eq!(quiet_for(&closed, 600), Standing::Closed);
    assert_eq!(quiet_for(&Run { ended: true, ..run }, 600), Standing::Ended);
}

#[test]
fn a_stop_time_that_cannot_be_read_reads_as_moving() {
    let run = Run { stopped: Some("sometime".into()), ..owed_run() };
    assert_eq!(run.standing_at(i64::MAX), Standing::Running);
}

#[test]
fn a_stop_says_how_long_ago_in_the_windows_ages() {
    let words = "the orchestrator's turn is over and no agent of the team works";
    assert_eq!(stopped_line(60), format!("stopped moving 1m ago: {words}"));
    assert_eq!(stopped_line(7200), format!("stopped moving 2h ago: {words}"));
    assert_eq!(stopped_line(3 * 86_400), format!("stopped moving 3d ago: {words}"));
}

#[test]
fn a_stage_not_run_since_the_last_change_is_owed() {
    let run = owed_run();
    assert_eq!([0, 1, 2].map(|stage| run.owes(stage)), [false, true, true]);
}

#[test]
fn a_start_answer_reads_free_builds() {
    let started: Started = serde_json::from_str(STARTED_FREE).expect("started");
    assert!(started.close_when_merged && started.free_builds);
    let plain: Started = serde_json::from_str(STARTED).expect("started");
    assert!(!plain.free_builds);
}

#[test]
fn an_age_below_zero_reads_as_now() {
    assert_eq!(age(-5), "now");
    assert_eq!(age(0), "now");
}

const EXITED: &str = crate::test_util::fixture!("flows/runs-exited.json");
const EXITED_OLD: &str = crate::test_util::fixture!("flows/runs-exited-0.13.70.json");

fn member(role: &str, pane: u64, exited: bool, closed: bool) -> Member {
    Member { role: role.into(), pane: Some(pane), exited, closed, ..Member::default() }
}

fn team(members: Vec<Member>) -> Run {
    Run { roles: members, ..Run::default() }
}

#[test]
fn a_role_whose_agent_exited_reads_as_exited() {
    let run = &parse_runs(EXITED).expect("runs").runs[0];
    let coder = run.member("coder").expect("coder");
    assert!(coder.exited && coder.agent_exited());
    assert_eq!(coder.status, None);
    assert!(!run.member("orchestrator").expect("orchestrator").exited);
    assert_eq!(run.exited(), ["coder"]);
}

#[test]
fn an_older_server_names_no_exited_role() {
    for text in [EXITED_OLD, RUNS] {
        let run = &parse_runs(text).expect("runs").runs[0];
        assert_eq!(run.exited(), Vec::<&str>::new());
        assert_eq!(run.exited_line(0), None);
    }
}

#[test]
fn a_closed_role_is_never_exited() {
    assert!(!member("coder", 8, true, true).agent_exited());
    let mut run = team(vec![member("coder", 8, true, false)]);
    run.state = Some("closed".into());
    assert_eq!(run.exited(), Vec::<&str>::new());
}

#[test]
fn an_ended_run_still_names_its_exited_roles() {
    let mut run = team(vec![member("coder", 8, true, false)]);
    run.ended = true;
    assert_eq!(run.exited(), ["coder"]);
}

#[test]
fn exited_roles_read_as_words() {
    let said = |names: &[&str]| {
        team(names.iter().zip(1..).map(|(n, p)| member(n, p, true, false)).collect()).exited_line(0).expect("a line")
    };
    assert_eq!(said(&["coder"]), "the agent of coder exited");
    assert_eq!(said(&["coder", "reviewer"]), "the agents of coder and reviewer exited");
    assert_eq!(said(&["coder", "reviewer", "tester"]), "the agents of coder, reviewer and tester exited");
}

#[test]
fn exited_roles_wait_for_the_workspaces_closed_agents() {
    let run = team(vec![member("coder", 8, true, false)]);
    assert_eq!(
        run.exited_line(1).as_deref(),
        Some("the agent of coder exited; Reopen brings back this workspace's closed agent first")
    );
    assert_eq!(
        run.exited_line(2).as_deref(),
        Some("the agent of coder exited; Reopen brings back this workspace's 2 closed agents first")
    );
}

#[test]
fn stop_goes_through_a_member_whose_agent_runs() {
    let run = team(vec![member("orchestrator", 7, true, false), member("coder", 8, false, false)]);
    assert_eq!((run.lead_pane(), run.stop_pane()), (Some(7), Some(8)));
    let all = team(vec![member("orchestrator", 7, true, false), member("coder", 8, true, false)]);
    assert_eq!((all.lead_pane(), all.stop_pane()), (Some(7), None));
}
