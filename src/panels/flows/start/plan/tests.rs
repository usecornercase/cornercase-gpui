use super::*;

fn issue() -> IssueTask {
    IssueTask {
        key: "#482".into(),
        source: "github".into(),
        title: "Returns page crashes".into(),
        branch: "issue-482-returns-page-crashes".into(),
    }
}

fn task(prompt: &str, issue: &str, branch: &str) -> Typed {
    Typed { prompt: prompt.into(), issue: issue.into(), branch: branch.into(), ..Typed::default() }
}

fn on_issue(branch: &str, workspace: &str) -> Typed {
    Typed { branch: branch.into(), workspace: workspace.into(), focus: true, ..Typed::default() }
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
    assert_eq!(plan.args(), ["flow", "run", "Quick", "--json", "--project", "3", "--prompt", "Fix the login form"]);
}

#[test]
fn an_issue_typed_in_the_dialog_is_the_task_and_a_typed_branch_names_the_worktree() {
    let typed = Typed { focus: true, ..task("", " ENG-42 ", "fix-login") };
    let plan = plan("Quick", None, None, &typed).expect("plans");
    assert_eq!(
        plan.args(),
        ["flow", "run", "Quick", "--json", "--issue", "ENG-42", "--worktree", "fix-login", "--focus"]
    );
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
        ["flow", "run", "Full issue", "--json", "--project", "4", "--issue", "#482", "--source", "github", "--focus"]
    );
}

#[test]
fn an_issue_sends_the_branch_and_the_workspace_name_once_they_are_edited() {
    let plan = plan("Quick", Some(4), Some(&issue()), &on_issue("feat/482-returns", "Returns crash")).expect("plans");
    assert_eq!(
        plan.args()[8..],
        ["--source", "github", "--worktree", "feat/482-returns", "--workspace-name", "Returns crash", "--focus"]
    );
}

#[test]
fn an_emptied_branch_or_workspace_name_keeps_the_issue_ones() {
    let plan = plan("Quick", Some(4), Some(&issue()), &on_issue(" ", "")).expect("plans");
    assert_eq!((plan.branch, plan.workspace_name), (None, None));
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
fn a_run_shown_in_the_window_closes_the_sheet_over_it_unless_the_flows_sheet_shows_it() {
    assert!(hides_the_run(true, Some(PanelKind::Issues)));
    assert!(!hides_the_run(true, Some(PanelKind::Flows)));
    assert!(!hides_the_run(false, Some(PanelKind::Issues)));
    assert!(!hides_the_run(true, None));
}
