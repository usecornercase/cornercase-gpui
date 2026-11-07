use super::{context_item, group_menu, open_in_item, plugin_items, projects_menu, setup_args, spot_of, workspace_menu};
use crate::context_menu::Entry;
use crate::model::{PaneInfo, Session};
use crate::panels::files::open::{Found, Launch, Opener};
use crate::panels::plugins::{Offers, Spot, listed_offers};
use crate::sidebar::Target;
use crate::sidebar::look::parse;

const PROJECT: &str = r#"{"groups":[{"id":7,"name":"team"}],"projects":[{"id":1,"name":"shop","path":"/home/ana/code/shop","workspaces":[
    {"id":2,"name":"plain","path":"/home/ana/notes"},
    {"id":3,"name":"main","path":"/home/ana/code/shop","branch":"main"},
    {"id":4,"name":"dark","path":"/home/ana/code/dark","branch":"dark","worktree":true,
        "pull_request":{"number":485,"url":"https://x/485","state":"open"}},
    {"id":5,"name":"fresh","path":"/home/ana/code/fresh","branch":"fresh","worktree":true,"setup":"running"},
    {"id":6,"name":"broken","path":"/home/ana/code/broken","branch":"broken","worktree":true,"setup":"failed"}]}]}"#;

const IMPORT: &str = "Import VS Code workspace…";

fn session() -> Session {
    serde_json::from_str(PROJECT).expect("a view line")
}

fn chosen(entries: &[Entry]) -> Vec<String> {
    entries
        .iter()
        .filter_map(|entry| match entry {
            Entry::Item { label, .. } | Entry::Submenu { label, .. } => Some(label.to_string()),
            Entry::Heading(_) | Entry::Resting { .. } | Entry::Separator => None,
        })
        .collect()
}

fn labels_with(workspace: u64, setup_of: &str) -> Vec<String> {
    let config = format!(r#"{{"projects":[{{"folder":"{setup_of}","setup":"npm ci"}}]}}"#);
    let look = parse(&config).expect("a config");
    chosen(&workspace_menu(&session(), workspace, &[], &look))
}

fn labels(workspace: u64) -> Vec<String> {
    labels_with(workspace, "/home/ana/code/shop")
}

fn offers_setup(labels: &[String]) -> bool {
    labels.iter().any(|label| label == "Run setup")
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
fn a_worktree_of_a_project_with_a_setup_offers_to_run_it_again() {
    assert!(offers_setup(&labels(4)));
}

#[test]
fn a_worktree_whose_setup_failed_offers_to_retry_it() {
    assert!(offers_setup(&labels(6)));
}

#[test]
fn run_setup_sits_after_ports_as_in_the_rest_of_the_workspaces_menu() {
    let labels = labels(4);
    let setup = labels.iter().position(|label| label == "Run setup").expect("the entry");
    assert_eq!(labels[setup - 1], "Ports…");
}

#[test]
fn a_worktree_still_running_its_setup_offers_no_second_run() {
    assert!(!offers_setup(&labels(5)));
}

#[test]
fn a_workspace_without_a_worktree_has_no_setup_to_run() {
    assert!(!offers_setup(&labels(3)));
}

#[test]
fn a_worktree_of_a_project_without_a_setup_has_none_to_run() {
    assert!(!offers_setup(&labels_with(4, "/home/ana/code/other")));
}

#[test]
fn running_a_setup_shows_its_tab_and_names_the_workspace() {
    assert_eq!(setup_args(4), ["setup", "--workspace", "4", "--json", "--focus"]);
}

#[test]
fn the_projects_menu_offers_importing_a_vs_code_workspace() {
    assert_eq!(chosen(&projects_menu()), ["Open project…", "New group…", IMPORT]);
}

#[test]
fn a_groups_menu_offers_importing_a_vs_code_workspace_after_adding_a_project() {
    let labels = chosen(&group_menu(&session(), 7));
    assert_eq!(labels[..2], ["Add project…", IMPORT]);
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
        Entry::Resting { label } => format!("({label})"),
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

fn opener_labels(openers: &[Found]) -> Option<(String, Vec<String>)> {
    let label = |entry: &Entry| match entry {
        Entry::Item { label, .. } | Entry::Submenu { label, .. } => label.to_string(),
        Entry::Heading(_) | Entry::Resting { .. } | Entry::Separator => String::new(),
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
