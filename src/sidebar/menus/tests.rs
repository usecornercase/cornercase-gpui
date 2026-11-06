use super::{context_item, plugin_items, spot_of, workspace_menu};
use crate::context_menu::Entry;
use crate::model::{PaneInfo, Session};
use crate::panels::plugins::{Offers, Spot, listed_offers};
use crate::sidebar::Target;

const PROJECT: &str = r#"{"projects":[{"id":1,"name":"shop","workspaces":[
    {"id":2,"name":"plain","path":"/home/ana/notes"},
    {"id":3,"name":"main","path":"/home/ana/code/shop","branch":"main"},
    {"id":4,"name":"dark","path":"/home/ana/code/dark","branch":"dark","worktree":true,
        "pull_request":{"number":485,"url":"https://x/485","state":"open"}}]}]}"#;

fn labels(workspace: u64) -> Vec<String> {
    let session: Session = serde_json::from_str(PROJECT).expect("a view line");
    let entries = workspace_menu(&session, workspace);
    entries
        .iter()
        .filter_map(|entry| match entry {
            Entry::Item { label, .. } | Entry::Submenu { label, .. } => Some(label.to_string()),
            Entry::Heading(_) | Entry::Separator => None,
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

fn plugin_row(entry: &Entry) -> String {
    match entry {
        Entry::Item { label, .. } | Entry::Submenu { label, .. } => label.to_string(),
        Entry::Heading(name) => format!("[{name}]"),
        Entry::Separator => "---".to_string(),
    }
}

#[test]
fn a_plugins_actions_come_below_a_line_under_its_name() {
    let entries = plugin_items(listed_offers().offered(Spot::Workspace(3)));
    let rows: Vec<String> = entries.iter().map(plugin_row).collect();
    assert_eq!(rows, ["---", "[Notes]", "Say where", "Fail on purpose", "---", "[Deploy]", "Deploy now"]);
}

#[test]
fn with_no_plugin_a_menu_gets_nothing() {
    assert!(plugin_items(Offers::default().offered(Spot::Pane(1))).is_empty());
}

#[test]
fn each_row_offers_the_actions_of_its_own_kind() {
    assert_eq!(spot_of(Target::Project(1)), Some(Spot::Project(1)));
    assert_eq!(spot_of(Target::Workspace(2)), Some(Spot::Workspace(2)));
    assert_eq!(spot_of(Target::Tab(3)), Some(Spot::Tab(3)));
}

#[test]
fn a_group_offers_no_actions() {
    assert_eq!(spot_of(Target::Group(1)), None);
}
