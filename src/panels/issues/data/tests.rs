use serde_json::json;

use super::*;

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
fn sources() {
    let sources: Sources = parse(SOURCES).expect("parses");
    assert_eq!(sources.project, 4);
    assert_eq!(sources.tabs().iter().map(|s| s.source.as_str()).collect::<Vec<_>>(), ["github", "jira"]);
    let github = sources.get("github").expect("github");
    assert!(github.connected() && github.repository);
    assert_eq!(github.people[0], Who::Me);
    assert_eq!(github.people[1], Who::Person { handle: "MiguelHang".into(), id: None });
    assert_eq!(github.account_line(), "as zamarrowski, through gh");
    let jira = sources.get("jira").expect("jira");
    assert!(!jira.connected());
    assert_eq!(jira.missing_line(), "needs a site, an email and a token");
    let shortcut = sources.get("shortcut").expect("shortcut");
    assert_eq!(shortcut.state, SourceState::Failing);
    assert!(shortcut.connected());
    assert_eq!(shortcut.account_line(), "token from $SHORTCUT_API_TOKEN");
    assert_eq!(sources.get("future").map(|s| s.state), Some(SourceState::Other));
    assert!(sources.any_connected());
    assert_eq!(sources.project_place().map(|p| p.label.as_str()), Some("cornercase-gpui"));
}

#[test]
fn missing_repository() {
    let info = SourceInfo { name: "GitLab".into(), missing: vec!["token".into(), "repository".into()], ..Default::default() };
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
    assert_eq!(people.people[0].label(), "Ana Ruiz (ana)");
    assert_eq!(people.people[1].label(), "bob");
    assert_eq!(people.people[0].who().arg(), "5b10");
}

#[test]
fn times() {
    assert_eq!(parse_time("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(parse_time("2026-10-22T20:13:09Z"), Some(1_792_699_989));
    assert_eq!(parse_time("2026-10-22T22:13:09.500+02:00"), Some(1_792_699_989));
    assert_eq!(parse_time("2026-10-22T22:13:09.000+0200"), Some(1_792_699_989));
    assert_eq!(parse_time("2026-10-22"), Some(1_792_627_200));
    assert_eq!(parse_time("yesterday"), None);
    let now = 1_792_699_989;
    assert_eq!(ago("2026-10-22T20:13:09Z", now), "just now");
    assert_eq!(ago("2026-10-22T19:13:09Z", now), "1h ago");
    assert_eq!(ago("2026-10-19T20:13:09Z", now), "3d ago");
    assert_eq!(ago("soon", now), "soon");
}

#[test]
fn agents_from_config() {
    let config = json!({"settings": [
        {"key": "agent", "value": "auto", "choices": [
            {"value": "auto", "note": "the agent in your tab"}, {"value": "claude", "note": "claude"},
            {"value": "codex", "note": "codex"}]},
        {"key": "agents.claude.mode", "value": "plan", "choices": [
            {"value": "default", "dangerous": false}, {"value": "plan"},
            {"value": "skip permissions (dangerous)", "dangerous": true}]},
        {"key": "agents.claude.extra", "value": ""}
    ]});
    let agents = Agents::from_config(&config);
    assert_eq!(agents.default, "auto");
    assert_eq!(agents.agents.iter().map(|a| a.value.as_str()).collect::<Vec<_>>(), ["claude", "codex"]);
    assert_eq!(agents.initial(), "claude");
    let (chosen, modes) = &agents.modes["claude"];
    assert_eq!(chosen, "plan");
    assert_eq!(modes.len(), 3);
    assert!(modes[2].dangerous);
    assert!(!agents.modes.contains_key("codex"));
    assert_eq!(Agents::from_config(&json!({})), Agents::default());
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
    let place = Place { project: 4, worktree: true, ..Default::default() };
    let plan = StartPlan {
        issue: issue(),
        agent: "claude".into(),
        mode: Some("plan".into()),
        model: Some("opus".into()),
        place: Some(place.clone()),
        workspace_name: Some("Popups".into()),
        ..Default::default()
    };
    assert_eq!(
        plan.start_args(),
        [
            "start", "claude", "--issue", "#456", "--source", "github", "--project", "4", "--mode", "plan", "--model",
            "opus", "--workspace-name", "Popups", "--json"
        ]
    );
    // The queue names the worktree's branch itself only when told.
    assert_eq!(
        plan.queue_args("in 2h"),
        [
            "queue", "add", "--at", "in 2h", "--issue", "#456", "--source", "github", "--agent", "claude", "--mode",
            "plan", "--project", "4", "--worktree", "issue-456-popups", "--name", "Popups"
        ]
    );
    let tab = StartPlan {
        issue: Issue { source: "linear".into(), key: "ENG-42".into(), ..Default::default() },
        agent: "codex".into(),
        effort: Some("high".into()),
        place: Some(Place { project: 9, workspace: Some(12), ..Default::default() }),
        ..Default::default()
    };
    assert_eq!(
        tab.start_args(),
        [
            "start", "codex", "--issue", "ENG-42", "--source", "linear", "--workspace", "12", "--effort", "high",
            "--json"
        ]
    );
    assert_eq!(
        tab.queue_args("claude reset"),
        [
            "queue", "add", "--at", "claude reset", "--issue", "ENG-42", "--source", "linear", "--agent", "codex",
            "--workspace", "12"
        ]
    );
}

#[test]
fn started_panes() {
    assert_eq!(started_pane(r#"{"ids": {"pane": 31, "tab": 9}}"#), Some(31));
    assert_eq!(started_pane(r#"{"pane": 7}"#), Some(7));
    assert_eq!(started_pane("12\n"), Some(12));
    assert_eq!(started_pane("nope"), None);
}

#[test]
fn info_without_chips() {
    let issue = Issue { state: "open".into(), labels: vec!["bug".into(), "ui".into()], ..Default::default() };
    let info: Vec<String> = ["open", "bug, ui", "assigned to @a", "opened by @b"].map(String::from).into();
    assert_eq!(issue.info_extras(&info), ["assigned to @a", "opened by @b"]);
}
