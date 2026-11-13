use super::*;
use crate::test_util::{fixture, shop_on_vps};

fn issue() -> IssueTask {
    IssueTask {
        key: "#482".into(),
        source: "github".into(),
        title: "Returns page crashes".into(),
        branch: "issue-482-returns-page-crashes".into(),
        prompt: Some("Fix https://github.com/o/r/issues/482\non issue-482-returns-page-crashes".into()),
    }
}

fn task(prompt: &str, issue: &str, branch: &str) -> Typed {
    Typed { prompt: prompt.into(), issue: issue.into(), branch: branch.into(), ..Typed::default() }
}

fn on_issue(branch: &str, workspace: &str) -> Typed {
    Typed { branch: branch.into(), workspace: workspace.into(), focus: true, ..Typed::default() }
}

fn retold(prompt: &str, branch: &str, workspace: &str) -> Typed {
    Typed { prompt: prompt.into(), ..on_issue(branch, workspace) }
}

fn made_there() -> IssueThere {
    serde_json::from_str(fixture!("machines/there_issue.json")).expect("what the machine answered")
}

fn open() -> Vec<(u64, String)> {
    vec![(1, "web".into()), (4, "api".into()), (7, "notes".into())]
}

fn ids(choices: &[(u64, String)]) -> Vec<u64> {
    choices.iter().map(|(id, _)| *id).collect()
}

#[test]
fn a_task_runs_as_the_prompt_in_the_chosen_project() {
    let plan = plan("Quick", Some(3), None, &task(" Fix the login form ", "", "")).expect("plans");
    assert_eq!(plan.args(), ["flow", "run", "Quick", "--json", "--project", "3", "--prompt=Fix the login form"]);
}

#[test]
fn an_issue_typed_in_the_dialog_is_the_task_and_a_typed_branch_names_the_worktree() {
    let typed = Typed { focus: true, ..task("", " ENG-42 ", "fix-login") };
    let plan = plan("Quick", None, None, &typed).expect("plans");
    assert_eq!(plan.args(), ["flow", "run", "Quick", "--json", "--issue=ENG-42", "--worktree=fix-login", "--focus"]);
}

#[test]
fn a_task_typed_beside_an_issue_replaces_the_issue_task() {
    let plan = plan("Quick", None, None, &task(" Fix the login form ", "ENG-42", "")).expect("plans");
    assert_eq!(plan.args(), ["flow", "run", "Quick", "--json", "--issue=ENG-42", "--prompt=Fix the login form"]);
}

#[test]
fn a_value_that_starts_with_a_hyphen_goes_joined_to_its_flag() {
    let plan = plan("Quick", None, None, &task("--help is wrong", "", "")).expect("plans");
    assert_eq!(plan.args()[4..], ["--prompt=--help is wrong"]);
}

#[test]
fn a_start_without_a_task_or_an_issue_is_refused() {
    let refused = plan("Quick", Some(3), None, &task(" ", "", "fix-login"));
    assert_eq!(refused, Err("Say what the team should do, or give an issue.".into()));
}

#[test]
fn an_issue_runs_on_its_key_and_tracker_and_leaves_untouched_names_to_cornercase() {
    let issue = issue();
    let plan = plan("Full issue", Some(4), Some(&issue), &on_issue(&issue.branch, &issue.workspace_name()));
    assert_eq!(
        plan.expect("plans").args(),
        ["flow", "run", "Full issue", "--json", "--project", "4", "--issue=#482", "--source=github", "--focus"]
    );
}

#[test]
fn an_issue_sends_the_branch_and_the_workspace_name_once_they_are_edited() {
    let plan = plan("Quick", Some(4), Some(&issue()), &on_issue("feat/482-returns", "Returns crash")).expect("plans");
    assert_eq!(
        plan.args()[7..],
        ["--source=github", "--worktree=feat/482-returns", "--workspace-name=Returns crash", "--focus"]
    );
}

#[test]
fn an_emptied_branch_or_workspace_name_keeps_the_issue_ones() {
    let plan = plan("Quick", Some(4), Some(&issue()), &on_issue(" ", "")).expect("plans");
    assert_eq!((plan.branch, plan.workspace_name), (None, None));
}

#[test]
fn the_issue_task_is_prefilled_with_its_line_breaks() {
    assert_eq!(issue().task(), "Fix https://github.com/o/r/issues/482\non issue-482-returns-page-crashes");
    let ended = IssueTask { prompt: Some("Fix it\n".into()), ..issue() };
    assert_eq!(ended.task(), "Fix it");
    assert_eq!(IssueTask { prompt: None, ..issue() }.task(), "");
}

#[test]
fn a_task_of_several_lines_runs_whole() {
    let plan = plan("Quick", Some(3), None, &task(" Fix the login form\nthen test it\n", "", "")).expect("plans");
    assert_eq!(plan.args().last().map(String::as_str), Some("--prompt=Fix the login form\nthen test it"));
}

#[test]
fn a_task_added_to_keeps_the_issues_lines() {
    let issue = issue();
    let typed = format!("{}\nTest first.", issue.task());
    let plan = plan("Quick", Some(4), Some(&issue), &retold(&typed, &issue.branch, "")).expect("plans");
    assert_eq!(plan.task, Task::Issue { key: "#482".into(), source: Some("github".into()), prompt: Some(typed) });
}

#[test]
fn a_task_left_as_prefilled_or_emptied_still_runs_on_the_issue() {
    let issue = issue();
    for prompt in [issue.task(), format!(" {} ", issue.task()), String::new()] {
        let plan = plan("Quick", Some(4), Some(&issue), &retold(&prompt, &issue.branch, "")).expect("plans");
        assert_eq!(plan.task, Task::Issue { key: "#482".into(), source: Some("github".into()), prompt: None });
    }
}

#[test]
fn an_edited_task_runs_on_the_issue_and_leaves_its_untouched_names_to_cornercase() {
    let issue = issue();
    let typed = retold(" Fix #482, test first ", &issue.branch, &issue.workspace_name());
    let plan = plan("Quick", Some(4), Some(&issue), &typed).expect("plans");
    assert_eq!(
        plan.args()[4..],
        ["--project", "4", "--issue=#482", "--source=github", "--prompt=Fix #482, test first", "--focus"]
    );
}

#[test]
fn an_edited_task_keeps_an_edited_branch_and_workspace_name() {
    let plan = plan("Quick", Some(4), Some(&issue()), &retold("Only the tests", "fix/482", "Returns crash"));
    let plan = plan.expect("plans");
    assert_eq!(
        plan.args()[8..],
        ["--prompt=Only the tests", "--worktree=fix/482", "--workspace-name=Returns crash", "--focus"]
    );
}

#[test]
fn a_task_typed_for_an_issue_whose_task_is_unknown_goes_with_the_issue() {
    let issue = IssueTask { prompt: None, ..issue() };
    let plan = plan("Quick", Some(4), Some(&issue), &retold("Fix it", "", "")).expect("plans");
    assert_eq!(
        plan.task,
        Task::Issue { key: "#482".into(), source: Some("github".into()), prompt: Some("Fix it".into()) }
    );
}

#[test]
fn a_cornercase_that_refuses_a_task_with_an_issue_is_named_too_old() {
    let clap = "the argument '--issue <KEY>' cannot be used with '--prompt <TEXT>'\n\nUsage: cornercase flow run";
    assert_eq!(refusal(clap.into()), TOO_OLD);
    assert_eq!(refusal("no flow named Quick".into()), "no flow named Quick");
}

#[test]
fn a_task_read_late_is_kept_as_the_issues() {
    let mut issue = IssueTask { prompt: None, ..issue() };
    issue.learn(Some("https://x/482".into()));
    assert_eq!(issue.prompt.as_deref(), Some("https://x/482"));
    issue.learn(None);
    assert_eq!(issue.prompt, None);
}

#[test]
fn the_issue_task_is_read_on_its_tracker_in_the_chosen_project() {
    assert_eq!(issue().read_args(Some(4)), ["issue", "read", "#482", "--source", "github", "--json", "--project", "4"]);
    assert_eq!(issue().read_args(None).len(), 6);
}

#[test]
fn the_task_is_the_prompt_of_the_read_issue_and_a_blank_one_is_unknown() {
    let read = r##"{"issue": {"key": "#482"}, "prompt": "https://x/482"}"##;
    assert_eq!(issue_prompt(read).expect("parses").as_deref(), Some("https://x/482"));
    assert_eq!(issue_prompt(r#"{"prompt": "  "}"#).expect("parses"), None);
    assert_eq!(issue_prompt(r#"{"issue": {}}"#).expect("parses"), None);
    assert!(issue_prompt("error: no issue #482").is_err());
}

#[test]
fn the_issue_prefills_its_workspace_with_its_key_and_title() {
    assert_eq!(issue().workspace_name(), "#482 Returns page crashes");
}

#[test]
fn a_flow_on_an_issue_needs_a_project() {
    let refused = plan("Quick", None, Some(&issue()), &on_issue("", ""));
    assert_eq!(refused, Err("Choose the project the team works in.".into()));
}

#[test]
fn a_project_asked_for_is_chosen_among_every_open_project() {
    let start = FlowStart::in_project(Some(4));
    assert_eq!(ids(&start.choices(&open())), [1, 4, 7]);
    assert_eq!(start.chosen(&open(), Some(1)), Some(4));
}

#[test]
fn without_a_project_asked_for_the_shown_one_then_the_first_is_chosen() {
    assert_eq!(FlowStart::default().chosen(&open(), Some(7)), Some(7));
    assert_eq!(FlowStart::default().chosen(&open(), None), Some(1));
    assert_eq!(FlowStart::in_project(Some(99)).chosen(&open(), Some(7)), Some(7));
}

#[test]
fn an_issue_keeps_to_the_projects_it_allows_without_guessing_one() {
    let start = FlowStart { projects: Some(vec![7, 4]), ..FlowStart::default() };
    let choices = start.choices(&open());
    assert_eq!(ids(&choices), [4, 7]);
    assert_eq!(start.chosen(&choices, Some(1)), None);
    assert_eq!(FlowStart { project: Some(7), ..start }.chosen(&choices, Some(1)), Some(7));
}

#[test]
fn each_picked_account_goes_with_its_agent() {
    let accounts = vec![
        AccountPick { agent: "claude".into(), folder: "work".into() },
        AccountPick { agent: "codex".into(), folder: String::new() },
    ];
    let typed = Typed { accounts, ..task("Fix it", "", "") };
    let args = plan("Quick", Some(4), None, &typed).expect("a plan").args();
    assert!(args.windows(4).any(|w| w == ["--account", "claude=work", "--account", "codex="]), "{args:?}");
}

#[test]
fn a_flow_on_a_machine_names_it_and_its_project_and_moves_no_window() {
    let args = plan("Quick", Some(4), None, &task("Fix it", "", "")).expect("a plan").args_there(&shop_on_vps());
    assert_eq!(args, ["flow", "run", "Quick", "--json", "--machine", "vps", "--project", "8", "--prompt=Fix it"]);
}

#[test]
fn a_flow_on_a_machine_keeps_its_issue_and_accounts() {
    let issue = IssueTask { key: "#7".into(), source: "github".into(), ..IssueTask::default() };
    let accounts = vec![AccountPick { agent: "claude".into(), folder: "home".into() }];
    let typed = Typed { accounts, ..task("", "", "") };
    let args = plan("Quick", Some(4), Some(&issue), &typed).expect("a plan").args_there(&shop_on_vps());
    assert!(args.windows(2).any(|pair| pair == ["--account", "claude=home"]), "{args:?}");
    assert!(args.contains(&"--issue=#7".to_string()), "{args:?}");
    assert!(args.contains(&"--source=github".to_string()), "{args:?}");
    assert!(!args.contains(&"--focus".to_string()), "{args:?}");
}

#[test]
fn a_flow_started_without_a_pick_names_no_account() {
    let args = plan("Quick", Some(4), None, &task("Fix it", "", "")).expect("a plan").args();
    assert!(!args.contains(&"--account".to_string()));
}

#[test]
fn an_issue_on_a_machine_prefills_what_its_templates_made() {
    let there = issue().there(Some(&made_there()));
    assert_eq!(there.branch, "ana/7-fix-the-login");
    assert_eq!(there.task(), "Fix #7: Fix the login\n\nRead CLAUDE.md before you start, then open a pull request.");
    assert_eq!(there.key, "#482");
    assert_eq!(there.workspace_name(), issue().workspace_name());
}

#[test]
fn an_issue_on_a_machine_not_read_yet_keeps_nothing_of_this_computer() {
    let there = issue().there(None);
    assert_eq!(there.prompt, None);
    assert_eq!(there.task(), "");
    assert_eq!(there.branch, "");
}

#[test]
fn a_task_and_branch_left_as_the_machine_made_them_are_not_sent() {
    let there = issue().there(Some(&made_there()));
    let typed = retold(&there.task(), &there.branch, &there.workspace_name());
    let args = plan("Quick", Some(4), Some(&there), &typed).expect("a plan").args_there(&shop_on_vps());
    assert!(args.contains(&"--issue=#482".to_string()), "{args:?}");
    assert!(!args.iter().any(|arg| arg.starts_with("--prompt") || arg.starts_with("--worktree")), "{args:?}");
}

#[test]
fn a_task_edited_over_the_machines_goes_as_typed() {
    let there = issue().there(Some(&made_there()));
    let typed = retold("Do it my way", "mine/7", &there.workspace_name());
    let args = plan("Quick", Some(4), Some(&there), &typed).expect("a plan").args_there(&shop_on_vps());
    assert!(args.contains(&"--prompt=Do it my way".to_string()), "{args:?}");
    assert!(args.contains(&"--worktree=mine/7".to_string()), "{args:?}");
}

#[test]
fn the_note_and_hints_name_whose_prompt_template_makes_the_task() {
    assert_eq!(issue_note(None), "The task starts as the issue's, through settings' prompt template.");
    assert_eq!(issue_note(Some("vps")), "The task starts as the issue's, through vps's prompt template.");
    assert_eq!(
        issue_hints(None),
        ("The issue's, through settings' prompt template".to_string(), "The issue's branch".to_string())
    );
    assert_eq!(
        issue_hints(Some("vps")),
        ("The issue's, through vps's prompt template".to_string(), "The issue's branch on vps".to_string())
    );
}

#[test]
fn a_start_that_closes_its_agents_once_merged_sends_the_flag() {
    let typed = Typed { close_when_merged: true, ..task("Fix it", "", "") };
    let plan = plan("Quick", Some(3), None, &typed).expect("plans");
    assert!(plan.args().contains(&"--close-when-merged".to_string()));
}

#[test]
fn a_start_left_open_sends_no_close_when_merged_flag() {
    let plan = plan("Quick", Some(3), None, &task("Fix it", "", "")).expect("plans");
    assert!(!plan.args().iter().any(|a| a == "--close-when-merged"));
}

#[test]
fn a_start_on_a_machine_sends_the_close_when_merged_flag_too() {
    let typed = Typed { close_when_merged: true, ..task("Fix it", "", "") };
    let plan = plan("Quick", Some(3), None, &typed).expect("plans");
    assert!(plan.args_there(&shop_on_vps()).contains(&"--close-when-merged".to_string()));
}

#[test]
fn a_start_that_frees_the_builds_sends_both_flags_in_order() {
    let typed = Typed { close_when_merged: true, free_builds: true, ..task("Fix it", "", "") };
    let args = plan("Quick", Some(3), None, &typed).expect("plans").args();
    let at = |flag: &str| args.iter().position(|a| a == flag).expect("the flag");
    assert_eq!(at("--free-builds"), at("--close-when-merged") + 1);
}

#[test]
fn free_builds_never_goes_without_the_close() {
    let typed = Typed { free_builds: true, ..task("Fix it", "", "") };
    let args = plan("Quick", Some(3), None, &typed).expect("plans").args();
    assert!(!args.iter().any(|a| a == "--free-builds" || a == "--close-when-merged"));
}

#[test]
fn a_start_on_a_machine_sends_the_free_builds_flag_too() {
    let typed = Typed { close_when_merged: true, free_builds: true, ..task("Fix it", "", "") };
    let plan = plan("Quick", Some(3), None, &typed).expect("plans");
    assert!(plan.args_there(&shop_on_vps()).contains(&"--free-builds".to_string()));
}
