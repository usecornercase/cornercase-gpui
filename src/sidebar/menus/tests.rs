use super::{context_item, open_in_item, workspace_menu};
use crate::context_menu::Entry;
use crate::model::{PaneInfo, Session};
use crate::panels::files::open::{Found, Launch, Opener};

const PROJECT: &str = r#"{"projects":[{"id":1,"name":"shop","workspaces":[
    {"id":2,"name":"plain","path":"/home/ana/notes"},
    {"id":3,"name":"main","path":"/home/ana/code/shop","branch":"main"},
    {"id":4,"name":"dark","path":"/home/ana/code/dark","branch":"dark","worktree":true,
        "pull_request":{"number":485,"url":"https://x/485","state":"open"}}]}]}"#;

fn labels(workspace: u64) -> Vec<String> {
    let session: Session = serde_json::from_str(PROJECT).expect("a view line");
    let entries = workspace_menu(&session, workspace, &[]);
    entries
        .iter()
        .filter_map(|entry| match entry {
            Entry::Item { label, .. } | Entry::Submenu { label, .. } => Some(label.to_string()),
            Entry::Separator => None,
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
fn a_pane_running_an_agent_offers_its_context() {
    let pane = PaneInfo { id: 1, agent: Some("claude".into()), ..PaneInfo::default() };
    assert!(context_item(&pane).is_some());
}

#[test]
fn a_pane_whose_agent_left_offers_no_context() {
    assert!(context_item(&PaneInfo::default()).is_none());
}

fn opener_labels(openers: &[Found]) -> Option<(String, Vec<String>)> {
    let label = |entry: &Entry| match entry {
        Entry::Item { label, .. } | Entry::Submenu { label, .. } => label.to_string(),
        Entry::Separator => String::new(),
    };
    open_in_item("/work/shop", openers).map(|entry| match &entry {
        Entry::Submenu { entries, .. } => (label(&entry), entries.iter().map(label).collect()),
        other => (label(other), Vec::new()),
    })
}

#[test]
fn a_folder_nothing_can_open_has_no_open_in() {
    assert_eq!(opener_labels(&[(Opener::DefaultApp, Launch::Desktop)]), None);
}

#[test]
fn one_app_is_an_entry_of_its_own() {
    let only = [(Opener::VsCode, Launch::Command("/bin/code".into()))];
    assert_eq!(opener_labels(&only), Some(("Open in VS Code".to_string(), Vec::new())));
}

#[test]
fn several_apps_share_a_submenu_without_the_default_app() {
    let found = [
        (Opener::VsCode, Launch::Command("/bin/code".into())),
        (Opener::Zed, Launch::Command("/bin/zed".into())),
        (Opener::DefaultApp, Launch::Desktop),
    ];
    let names = vec!["Open in VS Code".to_string(), "Open in Zed".to_string()];
    assert_eq!(opener_labels(&found), Some(("Open in…".to_string(), names)));
}
