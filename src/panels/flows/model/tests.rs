use super::*;

const RUNS: &str = crate::test_util::fixture!("flows/runs.json");
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
    let line = r#"{"revision":1,"runs":[{"run":2,"workspace":5,"flow":{"name":"Q","stages":[{"roles":["c"],"back":null}]},"roles":[],"stage":null,"rounds":[0],"state":"closed"}]}"#;
    let run = &parse_runs(line).expect("runs").runs[0];
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
