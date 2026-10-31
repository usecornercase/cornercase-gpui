use super::data::{Freed, List, Order, Tone, Worktree, free_command, free_message, list_command, shown, total};
use crate::panels::flows::model::epoch;
use crate::test_util::fixture;

fn worktrees() -> Vec<Worktree> {
    serde_json::from_str::<List>(fixture!("worktrees/list.json")).expect("the list parses").worktrees
}

fn named<'a>(list: &'a [Worktree], branch: &str) -> &'a Worktree {
    list.iter().find(|w| w.branch.as_deref() == Some(branch)).expect("the branch is listed")
}

#[test]
fn a_risky_worktree_shows_its_changes_commits_and_programs() {
    let list = worktrees();
    let marks: Vec<String> = named(&list, "feat/try").marks().into_iter().map(|(m, _)| m).collect();
    assert_eq!(marks, ["●1", "↑1", "Python", "open"]);
}

#[test]
fn a_worktree_without_commits_says_so_quietly() {
    let list = worktrees();
    assert_eq!(named(&list, "feat/merged").marks()[0], ("no commits".to_string(), Tone::Plain));
}

#[test]
fn a_leftover_names_its_kind() {
    let list = worktrees();
    assert_eq!(named(&list, "old/thing").marks()[0], ("prunable".to_string(), Tone::Warn));
}

#[test]
fn a_leftover_cornercase_could_not_scan_is_still_listed() {
    let list = serde_json::from_str::<List>(fixture!("worktrees/leftovers.json")).expect("the list parses").worktrees;
    assert_eq!(list[0].marks(), [("repository gone".to_string(), Tone::Warn)]);
    assert_eq!(list[0].size_label(false), "8.0 KB");
    assert_eq!(list[0].remove_command(), None);
}

#[test]
fn the_size_names_the_part_in_build_folders() {
    let list = worktrees();
    assert_eq!(named(&list, "feat/try").size_label(false), "308.0 KB · 296.0 KB builds");
    assert_eq!(named(&list, "feat/merged").size_label(false), "8.0 KB");
    assert_eq!(named(&list, "old/thing").size_label(false), "?");
    assert_eq!(Worktree::default().size_label(true), "…");
}

#[test]
fn only_an_open_workspace_can_be_removed_here() {
    let list = worktrees();
    assert!(named(&list, "feat/try").removable());
    assert!(!named(&list, "old/thing").removable());
    assert_eq!(named(&list, "old/thing").remove_command(), None);
}

#[test]
fn removing_forces_only_when_files_changed() {
    let list = worktrees();
    let risky = named(&list, "feat/try").remove_command().expect("removable");
    assert_eq!(risky, ["close", "--workspace", "22", "--remove-worktree", "--force"]);
    let clean = named(&list, "feat/merged").remove_command().expect("removable");
    assert_eq!(clean, ["close", "--workspace", "25", "--remove-worktree"]);
}

#[test]
fn the_filter_matches_project_or_branch() {
    let list = worktrees();
    assert_eq!(shown(&list, "TRY", Order::SafeFirst).len(), 1);
    assert_eq!(shown(&list, "shop", Order::SafeFirst).len(), 1);
    assert_eq!(shown(&list, "", Order::SafeFirst).len(), 3);
}

#[test]
fn largest_first_puts_unmeasured_worktrees_last() {
    let list = worktrees();
    let order: Vec<&str> =
        shown(&list, "", Order::LargestFirst).into_iter().filter_map(|i| list[i].branch.as_deref()).collect();
    assert_eq!(order, ["feat/try", "feat/merged", "old/thing"]);
}

#[test]
fn the_total_adds_up_and_marks_unknown_sizes() {
    let list = worktrees();
    let now = epoch("2026-10-29T21:00:00.000Z").expect("a time");
    assert_eq!(total(&list, now), "316.0 KB+ in 3 worktrees");
}

#[test]
fn the_total_names_builds_left_idle_for_a_week() {
    let list = worktrees();
    let later = epoch("2026-11-10T00:00:00.000Z").expect("a time");
    assert!(total(&list, later).ends_with("296.0 KB of builds idle 7+ days"));
}

#[test]
fn the_commands_ask_for_json() {
    assert_eq!(list_command(false), ["worktrees", "list", "--json", "--no-sizes"]);
    assert_eq!(free_command("/w", true, false), ["worktrees", "free-builds", "--path", "/w", "--json", "--check"]);
    assert_eq!(free_command("/w", false, true), ["worktrees", "free-builds", "--path", "/w", "--json", "--force"]);
}

#[test]
fn freeing_warns_about_what_runs_in_the_worktree() {
    let freed: Freed = serde_json::from_str(fixture!("worktrees/free-check.json")).expect("the check parses");
    let text = free_message(&freed);
    assert!(text.starts_with("Frees 296.0 KB: node_modules 296.0 KB."));
    assert!(text.ends_with("Python runs in it and may still use them."));
}
