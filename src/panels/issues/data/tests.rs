use super::*;
use crate::model::machines::parse_list;
use crate::model::machines::run_on::server_release;
use crate::test_util::{fixture, shop_on_vps};

const SOURCES: &str = r#"{
  "closed": false,
  "places": [
    {"here": true, "label": "cornercase-gpui", "project": 4, "workspace": null, "worktree": true},
    {"here": false, "label": "notes › main", "project": 9, "workspace": 12, "worktree": false}
  ],
  "project": 4,
  "sources": [
    {"fields": ["assignee", "author"], "from": "gh", "account": "zamarrowski", "missing": [], "name": "GitHub",
     "people": ["me", {"person": {"handle": "MiguelHang", "id": null}}], "repository": true,
     "source": "github", "state": "connected", "tab": true},
    {"fields": ["assignee", "reporter"], "missing": ["site", "email", "token"], "name": "Jira",
     "people": ["anyone", "anyone"], "repository": false, "source": "jira", "state": "missing", "tab": true},
    {"fields": ["owner", "requester"], "missing": [], "name": "Shortcut", "reason": "token rejected",
     "people": ["anyone", "anyone"], "repository": false, "source": "shortcut", "state": "failing", "tab": false,
     "from": "env", "env": "SHORTCUT_API_TOKEN"},
    {"source": "future", "state": "something-new"}
  ]
}"#;

#[test]
fn starting_on_a_machine_names_its_project_and_moves_no_window() {
    let plan = StartPlan {
        issue: Issue { key: "#7".into(), source: "github".into(), ..Issue::default() },
        agent: "claude".into(),
        place: Some(Place { project: 4, workspace: Some(3), ..Place::default() }),
        branch: Some("fix-it".into()),
        accounts: vec![AccountPick { agent: "claude".into(), folder: "home".into() }],
        ..StartPlan::default()
    };
    assert_eq!(
        plan.start_args_there(&shop_on_vps()),
        [
            "start",
            "claude",
            "--issue",
            "#7",
            "--source",
            "github",
            "--machine",
            "vps",
            "--project",
            "8",
            "--worktree",
            "fix-it",
            "--account",
            "claude=home",
            "--placed",
            "--json"
        ]
    );
}

#[test]
fn a_start_here_says_it_is_starting_and_shows_its_tab_from_the_answer() {
    let start = worktree_plan().start_here();
    assert_eq!(start.doing, "Starting claude on #456…");
    assert_eq!(start.done, "claude is starting on #456");
    assert_eq!(start.failed, "Could not start #456");
    assert_eq!(start.command, worktree_plan().start_args());
    assert!(start.command.ends_with(&["--focus".into(), "--placed".into(), "--json".into()]));
}

#[test]
fn sources() {
    let sources: Sources = parse(SOURCES).expect("parses");
    assert_eq!(sources.project, 4);
    assert_eq!(sources.tabs().iter().map(|s| s.source.as_str()).collect::<Vec<_>>(), ["github", "jira"]);
    check_github(sources.get("github").expect("github"));
    check_missing_and_failing(&sources);
    assert_eq!(sources.get("future").map(|s| s.state), Some(SourceState::Other));
    assert!(sources.any_connected());
    assert_eq!(sources.project_place().map(|p| p.label.as_str()), Some("cornercase-gpui"));
}

fn check_github(github: &SourceInfo) {
    assert!(github.connected() && github.repository);
    assert_eq!(github.people[0], Who::Me);
    assert_eq!(github.people[1], Who::Person { handle: "MiguelHang".into(), id: None });
    assert_eq!(github.account_line(), "as zamarrowski, through gh");
}

fn check_missing_and_failing(sources: &Sources) {
    let jira = sources.get("jira").expect("jira");
    assert!(!jira.connected());
    assert_eq!(jira.missing_line(), "needs a site, an email and a token");
    let shortcut = sources.get("shortcut").expect("shortcut");
    assert_eq!(shortcut.state, SourceState::Failing);
    assert!(shortcut.connected());
    assert_eq!(shortcut.account_line(), "token from $SHORTCUT_API_TOKEN");
}

#[test]
fn missing_repository() {
    let info =
        SourceInfo { name: "GitLab".into(), missing: vec!["token".into(), "repository".into()], ..Default::default() };
    assert_eq!(info.missing_line(), "this project's folder is not a GitLab repository");
    let one = SourceInfo { missing: vec!["token".into()], ..Default::default() };
    assert_eq!(one.missing_line(), "needs a token");
}

#[test]
fn who_arguments() {
    assert_eq!(Who::Anyone.arg(), "anyone");
    assert_eq!(Who::Me.arg(), "me");
    let jira = Who::Person { handle: "Ana Ruiz".into(), id: Some("5b10ac8d".into()) };
    assert_eq!((jira.arg(), jira.label()), ("5b10ac8d".into(), "Ana Ruiz".into()));
    assert!(jira.is_person() && !Who::Me.is_person());
}

#[test]
fn list_with_a_failing_source() {
    let out = r##"{"issues": [
        {"assignees": [], "author": "a", "branch": "issue-1-x", "closed": false, "key": "#1", "labels": ["bug"],
         "number": 1, "source": "github", "state": "open", "title": "Old", "updated_at": "2026-10-01T10:00:00Z",
         "url": "https://github.com/o/r/issues/1"},
        {"key": "ENG-2", "source": "linear", "title": "New login", "updated_at": "2026-10-20T10:00:00.123+02:00",
         "assignees": ["ana"], "labels": ["Frontend"]}
      ],
      "project": 8,
      "sources": [{"cached": false, "count": 1, "source": "github"},
                  {"cached": false, "count": 0, "error": "Plane is not connected", "source": "plane"}]}"##;
    let mut lists: Lists = parse(out).expect("parses");
    assert_eq!(lists.sources[1].error.as_deref(), Some("Plane is not connected"));
    sort_newest_first(&mut lists.issues);
    assert_eq!(lists.issues[0].key, "ENG-2");
    assert!(lists.issues[0].matches("login front"));
    assert!(lists.issues[0].matches("ANA"));
    assert!(!lists.issues[0].matches("login backend"));
    assert!(lists.issues[1].matches("#1 bug"));
    assert!(parse::<Lists>("Plane: Plane is not connected").is_none());
}

#[test]
fn read_and_people() {
    let read: ReadIssue = parse(
        r###"{"body": "## Problem", "comments": [{"author": "b", "created_at": "2026-10-22T08:00:00Z", "body": "+1"}],
             "info": ["open", "assigned to @a"], "issue": {"key": "#456", "source": "github"}, "prompt": "https://x"}"###,
    )
    .expect("parses");
    assert_eq!((read.issue.key.as_str(), read.comments.len(), read.info.len()), ("#456", 1, 2));
    let people: People = parse(
        r#"{"cached": false, "fields": ["assignee", "reporter"], "source": "jira",
            "people": [{"handle": "ana", "id": "5b10", "name": "Ana Ruiz"}, {"handle": "bob", "id": null, "name": ""}]}"#,
    )
    .expect("parses");
    assert_eq!(people.list[0].label(), "Ana Ruiz (ana)");
    assert_eq!(people.list[1].label(), "bob");
    assert_eq!(people.list[0].who().arg(), "5b10");
}

#[test]
fn times() {
    assert_eq!(parse_time("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(parse_time("2026-10-22T20:13:09Z"), Some(1_792_699_989));
    assert_eq!(parse_time("2026-10-22T22:13:09.500+02:00"), Some(1_792_699_989));
    assert_eq!(parse_time("2026-10-22T22:13:09.000+0200"), Some(1_792_699_989));
    assert_eq!(parse_time("2026-10-22"), Some(1_792_627_200));
    assert_eq!(parse_time("yesterday"), None);
    check_ago();
}

fn check_ago() {
    let now = 1_792_699_989;
    assert_eq!(ago("2026-10-22T20:13:09Z", now), "just now");
    assert_eq!(ago("2026-10-22T19:13:09Z", now), "1h ago");
    assert_eq!(ago("2026-10-19T20:13:09Z", now), "3d ago");
    assert_eq!(ago("soon", now), "soon");
}

#[test]
fn schedule_times() {
    assert_eq!(When::At.at(" 14:30 "), Some("14:30".into()));
    assert_eq!(When::Tomorrow.at("9:00"), Some("tomorrow 9:00".into()));
    assert_eq!(When::On.at("2026-10-20 09:00"), Some("2026-10-20 09:00".into()));
    assert_eq!(When::In.at("2 h"), Some("in 2h".into()));
    assert_eq!(When::In.at("in 30m"), Some("in 30m".into()));
    assert_eq!(When::In.at(""), None);
    assert_eq!(When::ClaudeReset.at(""), Some("claude reset".into()));
    assert_eq!(When::CodexReset.at("ignored"), Some("codex reset".into()));
}

fn issue() -> Issue {
    Issue { source: "github".into(), key: "#456".into(), branch: "issue-456-popups".into(), ..Default::default() }
}

#[test]
fn start_arguments() {
    check_worktree_plan();
    check_worktree_queue();
    check_tab_plan();
    check_tab_queue();
}

fn worktree_plan() -> StartPlan {
    StartPlan {
        issue: issue(),
        agent: "claude".into(),
        mode: Some("plan".into()),
        model: Some("opus".into()),
        place: Some(Place { project: 4, worktree: true, ..Default::default() }),
        workspace_name: Some("Popups".into()),
        ..Default::default()
    }
}

fn tab_plan() -> StartPlan {
    StartPlan {
        issue: Issue { source: "linear".into(), key: "ENG-42".into(), ..Default::default() },
        agent: "codex".into(),
        effort: Some("high".into()),
        place: Some(Place { project: 9, workspace: Some(12), ..Default::default() }),
        ..Default::default()
    }
}

fn check_worktree_plan() {
    let plan = worktree_plan();
    assert_eq!(
        plan.start_args(),
        [
            "start",
            "claude",
            "--issue",
            "#456",
            "--source",
            "github",
            "--project",
            "4",
            "--mode",
            "plan",
            "--model",
            "opus",
            "--workspace-name",
            "Popups",
            "--focus",
            "--placed",
            "--json"
        ]
    );
}

fn check_worktree_queue() {
    let plan = worktree_plan();
    assert_eq!(
        plan.queue_args("in 2h"),
        [
            "queue",
            "add",
            "--at",
            "in 2h",
            "--issue",
            "#456",
            "--source",
            "github",
            "--agent",
            "claude",
            "--mode",
            "plan",
            "--project",
            "4",
            "--worktree",
            "issue-456-popups",
            "--name",
            "Popups"
        ]
    );
}

fn check_tab_plan() {
    let tab = tab_plan();
    assert_eq!(
        tab.start_args(),
        [
            "start",
            "codex",
            "--issue",
            "ENG-42",
            "--source",
            "linear",
            "--workspace",
            "12",
            "--effort",
            "high",
            "--focus",
            "--placed",
            "--json"
        ]
    );
}

fn check_tab_queue() {
    let tab = tab_plan();
    assert_eq!(
        tab.queue_args("claude reset"),
        [
            "queue",
            "add",
            "--at",
            "claude reset",
            "--issue",
            "ENG-42",
            "--source",
            "linear",
            "--agent",
            "codex",
            "--workspace",
            "12"
        ]
    );
}

#[test]
fn info_without_chips() {
    let issue = Issue { state: "open".into(), labels: vec!["bug".into(), "ui".into()], ..Default::default() };
    let info: Vec<String> = ["open", "bug, ui", "assigned to @a", "opened by @b"].map(String::from).into();
    assert_eq!(issue.info_extras(&info), ["assigned to @a", "opened by @b"]);
}

#[test]
fn the_count_says_how_many_issues_the_search_leaves() {
    assert_eq!(count_line(20, 20, false), "20 open issues");
    assert_eq!(count_line(1, 1, false), "1 open issue");
    assert_eq!(count_line(3, 20, false), "3 of 20 open issues");
    assert_eq!(count_line(7, 7, true), "7 issues");
    assert_eq!(count_line(0, 1, true), "0 of 1 issue");
}

#[test]
fn a_state_label_names_open_and_closed_and_keeps_a_tracker_state() {
    let open = Issue { state: "open".into(), ..Default::default() };
    let closed = Issue { state: "closed".into(), closed: true, ..Default::default() };
    let unnamed = Issue { closed: true, ..Default::default() };
    let linear = Issue { state: "In Progress".into(), ..Default::default() };
    assert_eq!(
        [&open, &closed, &unnamed, &linear].map(Issue::state_label),
        ["Open", "Closed", "Closed", "In Progress"]
    );
    assert_eq!([&open, &closed, &unnamed, &linear].map(Issue::state_note), [None, None, None, Some("In Progress")]);
}

#[test]
fn a_tracker_status_says_how_it_connects_or_what_it_needs() {
    let sources: Sources = parse(SOURCES).expect("parses");
    let line = |source: &str| sources.get(source).expect(source).status_line();
    assert_eq!(line("github"), "Connected as zamarrowski, through gh");
    assert_eq!(line("jira"), "Needs a site, an email and a token");
    assert_eq!(line("shortcut"), "Token rejected");
    assert_eq!(line("future"), "Not connected");
}

#[test]
fn a_person_shows_the_name_and_a_different_handle() {
    let person = |name: &str, handle: &str| Person { name: name.into(), handle: handle.into(), id: None };
    assert_eq!(person("Ana Ruiz", "ana").display(), ("Ana Ruiz".into(), Some("ana".into())));
    assert_eq!(person("bob", "bob").display(), ("bob".into(), None));
    assert_eq!(person("", "carla").display(), ("carla".into(), None));
}

fn root(project: u64, here: bool) -> Place {
    Place { project, workspace: None, label: format!("p{project}"), worktree: true, here }
}

fn tab_place(project: u64, workspace: u64, here: bool) -> Place {
    Place { project, workspace: Some(workspace), label: format!("w{workspace}"), worktree: false, here }
}

fn with_places(project: u64, places: Vec<Place>) -> Sources {
    Sources { project, places, ..Default::default() }
}

#[test]
fn a_flow_on_a_repository_issue_runs_in_the_issue_project_on_its_branch() {
    let issue = Issue { title: "Popups".into(), ..issue() };
    let start = flow_start(&issue, &with_places(4, vec![root(5, true), tab_place(4, 12, false)]), true, None);
    assert_eq!((start.project, start.projects), (Some(4), Some(vec![4])));
    let task = start.issue.expect("an issue");
    assert_eq!([task.key, task.source, task.title, task.branch], ["#456", "github", "Popups", "issue-456-popups"]);
}

#[test]
fn a_flow_on_another_tracker_issue_offers_the_repository_roots_and_chooses_the_sheet_project() {
    let sources = with_places(5, vec![root(4, true), tab_place(9, 12, false), root(5, false)]);
    let start = flow_start(&tab_plan().issue, &sources, false, None);
    assert_eq!((start.project, start.projects), (Some(5), Some(vec![4, 5])));
}

#[test]
fn the_only_repository_root_is_chosen_and_several_leave_the_choice_open_whatever_cornercase_marks_here() {
    let only =
        flow_start(&tab_plan().issue, &with_places(9, vec![root(4, false), tab_place(9, 12, true)]), false, None);
    assert_eq!(only.project, Some(4));
    let several = with_places(9, vec![root(4, true), root(5, false), tab_place(9, 12, false)]);
    assert_eq!(flow_start(&tab_plan().issue, &several, false, None).project, None);
}

#[test]
fn a_flow_on_an_issue_takes_the_task_of_its_read_page_and_leaves_a_blank_one_to_read() {
    let sources = with_places(4, vec![root(4, true)]);
    let read = ReadIssue { prompt: "https://x/456".into(), ..ReadIssue::default() };
    let task = |read: &ReadIssue| flow_start(&issue(), &sources, true, Some(read)).issue.expect("an issue").prompt;
    assert_eq!(task(&read).as_deref(), Some("https://x/456"));
    assert_eq!(task(&ReadIssue { prompt: " ".into(), ..read }), None);
    assert_eq!(flow_start(&issue(), &sources, true, None).issue.expect("an issue").prompt, None);
}

fn on_work() -> Vec<AccountPick> {
    vec![AccountPick { agent: "claude".into(), folder: "work".into() }]
}

#[test]
fn a_start_with_a_picked_account_names_it() {
    let args = StartPlan { accounts: on_work(), ..worktree_plan() }.start_args();
    assert!(args.windows(2).any(|pair| pair == ["--account", "claude=work"]), "{args:?}");
}

#[test]
fn a_scheduled_start_with_a_picked_account_names_it() {
    let args = StartPlan { accounts: on_work(), ..worktree_plan() }.queue_args("in 2h");
    assert!(args.windows(2).any(|pair| pair == ["--account", "claude=work"]), "{args:?}");
}

#[test]
fn a_start_without_a_pick_names_no_account() {
    assert!(!worktree_plan().start_args().contains(&"--account".to_string()));
    assert!(!worktree_plan().queue_args("in 2h").contains(&"--account".to_string()));
}

fn made_there() -> IssueThere {
    serde_json::from_str(fixture!("machines/there_issue.json")).expect("what the machine answered")
}

#[test]
fn starting_on_a_machine_sends_the_mode_picked_there() {
    let plan = StartPlan {
        issue: Issue { key: "#7".into(), source: "github".into(), ..Issue::default() },
        agent: "claude".into(),
        mode: Some("auto".into()),
        place: Some(Place { project: 4, ..Place::default() }),
        ..StartPlan::default()
    };
    let args = plan.start_args_there(&shop_on_vps());
    let at = args.iter().position(|arg| arg == "--project").expect("the project");
    assert_eq!(args[at + 1], "8");
    assert!(args.windows(2).any(|pair| pair == ["--mode", "auto"]), "{args:?}");
}

#[test]
fn the_footnote_on_a_machine_quotes_the_prompt_its_template_made() {
    assert_eq!(
        there_footnote("vps", Some(&made_there())),
        "The agent starts with vps's prompt for the issue, sent at once: “Fix #7: Fix the login Read CLAUDE.md before \
         you start, then open a pull request.”. Schedule is hidden: the queue starts work on this computer only."
    );
}

#[test]
fn the_footnote_on_a_machine_falls_back_to_its_template_line() {
    let today = "The agent starts with the issue as its first prompt (vps's prompt template, the issue's URL by \
                 default), sent at once. Schedule is hidden: the queue starts work on this computer only.";
    assert_eq!(there_footnote("vps", None), today);
    let blank = IssueThere { prompt: "  \n".into(), ..IssueThere::default() };
    assert_eq!(there_footnote("vps", Some(&blank)), today);
}

#[test]
fn the_branch_hint_names_the_machine() {
    assert_eq!(branch_hint(None), "the issue's branch");
    assert_eq!(branch_hint(Some("vps")), "the issue's branch on vps");
}

fn here_agents() -> Agents {
    Agents::from_config(&serde_json::json!({"settings": [
        {"key": "agents.claude.mode", "value": "plan", "choices": [{"value": "default"}, {"value": "plan"}]}
    ]}))
}

fn there_agents() -> Agents {
    serde_json::from_str(fixture!("machines/there_config.json")).expect("what the machine answered")
}

#[test]
fn a_machine_not_heard_from_shows_no_modes_and_no_branch_of_this_computers() {
    let here = here_agents();
    assert!(modes_shown("claude", &here, Source::Here).is_some());
    assert_eq!(modes_shown("claude", &here, Source::There(None)), None);
    assert_eq!(branch_shown("issue-7-here", Source::There(None)), "");
    assert_eq!(branch_shown("issue-7-here", Source::Here), "issue-7-here");
}

#[test]
fn a_machine_that_answered_shows_its_own_modes_and_branch() {
    let there = there_agents();
    let modes = modes_shown("claude", &here_agents(), Source::There(Some(&there))).expect("its modes");
    assert_eq!(modes.choices.len(), 5);
    assert_eq!(branch_shown("issue-7-here", Source::There(Some(&made_there()))), "ana/7-fix-the-login");
}

#[test]
fn a_mode_picked_on_a_machine_is_sent() {
    let there = there_agents();
    let modes = modes_shown("claude", &here_agents(), Source::There(Some(&there))).expect("its modes");
    assert_eq!(modes.sent(Some("auto")).as_deref(), Some("auto"));
}

#[test]
fn default_picked_goes_to_start_here_there_and_to_the_queue() {
    let plan = StartPlan {
        agent: "claude".into(),
        mode: Some("default".into()),
        place: Some(Place { project: 4, ..Place::default() }),
        ..StartPlan::default()
    };
    let pair = ["--mode", "default"];
    assert!(plan.start_args().windows(2).any(|args| args == pair));
    assert!(plan.start_args_there(&shop_on_vps()).windows(2).any(|args| args == pair));
    assert!(plan.queue_args("in 2h").windows(2).any(|args| args == pair));
}

#[test]
fn a_machine_whose_server_starts_default_offers_it_and_one_behind_rests_it() {
    let list = parse_list(fixture!("machines/list_mode_default.json")).expect("the list").machines;
    let there = there_agents();
    let source = || modes_shown("claude", &here_agents(), Source::There(Some(&there))).expect("its modes");
    let vps = source().on_server(server_release(Some("vps"), None, &list));
    assert_eq!(vps.sent(Some("default")).as_deref(), Some("default"));
    let lab = source().on_server(server_release(Some("lab"), None, &list));
    assert_eq!(lab.sent(Some("default")), None);
    let line = lab.waiting(Some("lab")).expect("the line");
    assert!(line.contains("on lab") && line.contains("lab runs 0.13.21"), "{line}");
}
