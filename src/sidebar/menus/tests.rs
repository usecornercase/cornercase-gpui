use super::{context_item, projects_menu, workspace_menu};
use crate::context_menu::Entry;
use crate::model::{PaneInfo, Session};

const PROJECT: &str = r#"{"projects":[{"id":1,"name":"shop","workspaces":[
    {"id":2,"name":"plain","path":"/home/ana/notes"},
    {"id":3,"name":"main","path":"/home/ana/code/shop","branch":"main"},
    {"id":4,"name":"dark","path":"/home/ana/code/dark","branch":"dark","worktree":true,
        "pull_request":{"number":485,"url":"https://x/485","state":"open"}},
    {"id":5,"name":"fresh","path":"/home/ana/code/fresh","branch":"fresh","worktree":true,"setup":"running"},
    {"id":6,"name":"broken","path":"/home/ana/code/broken","branch":"broken","worktree":true,"setup":"failed"}]}]}"#;

fn menu(workspace: u64) -> Vec<Entry> {
    let session: Session = serde_json::from_str(PROJECT).expect("a view line");
    workspace_menu(&session, workspace)
}

fn labels(workspace: u64) -> Vec<String> {
    menu(workspace)
        .iter()
        .filter_map(|entry| match entry {
            Entry::Item { label, .. } | Entry::Submenu { label, .. } => Some(label.to_string()),
            Entry::Note(_) | Entry::Separator => None,
        })
        .collect()
}

fn notes(entries: &[Entry]) -> Vec<String> {
    entries
        .iter()
        .filter_map(|entry| match entry {
            Entry::Note(text) => Some(text.to_string()),
            _ => None,
        })
        .collect()
}

fn offers_the_check(labels: &[String]) -> bool {
    labels.iter().any(|label| label == "Merge check…")
}

#[test]
fn a_workspace_on_a_branch_offers_the_merge_check_with_nothing_shared() {
    assert!(offers_the_check(&labels(3)));
}

#[test]
fn a_plain_folder_offers_no_merge_check() {
    assert!(!offers_the_check(&labels(2)));
}

#[test]
fn the_merge_check_comes_after_the_pull_request() {
    let labels = labels(4);
    let check = labels.iter().position(|label| label == "Merge check…").expect("the entry");
    assert!(labels[check - 1].starts_with("Pull request #485"), "{labels:?}");
}

#[test]
fn a_worktree_says_what_running_its_setup_again_waits_on() {
    assert_eq!(notes(&menu(4)), ["Run setup\nneeds usecornercase/cornercase-terminal#672"]);
}

#[test]
fn a_worktree_whose_setup_failed_says_what_retrying_it_waits_on() {
    assert_eq!(notes(&menu(6)), ["Run setup\nneeds usecornercase/cornercase-terminal#672"]);
}

#[test]
fn a_worktree_still_running_its_setup_has_nothing_to_say_about_it() {
    assert_eq!(notes(&menu(5)), Vec::<String>::new());
}

#[test]
fn a_workspace_without_a_worktree_has_no_setup_to_run() {
    assert_eq!(notes(&menu(3)), Vec::<String>::new());
}

#[test]
fn the_projects_menu_says_what_importing_a_vs_code_workspace_waits_on() {
    assert_eq!(notes(&projects_menu()), ["Importing a VS Code workspace\nneeds usecornercase/cornercase-terminal#673"]);
}

#[test]
fn a_pane_running_an_agent_offers_its_context() {
    let pane = PaneInfo { id: 1, agent: Some("claude".into()), ..PaneInfo::default() };
    assert!(context_item(&pane).is_some());
}

#[test]
fn a_pane_whose_agent_left_offers_no_context() {
    assert!(context_item(&PaneInfo::default()).is_none());
}
