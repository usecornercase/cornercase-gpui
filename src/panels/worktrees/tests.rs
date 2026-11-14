use super::data::{
    List, MergedBuilds, Order, Tone, Worktree, last_folder, list_command, name_of, scope_note, shown, totals,
};
use crate::panels::flows::model::epoch;
use crate::test_util::fixture;

fn list() -> List {
    serde_json::from_str::<List>(fixture!("worktrees/list.json")).expect("the list parses")
}

fn worktrees() -> Vec<Worktree> {
    list().worktrees
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
fn every_kind_of_row_can_be_removed() {
    let list = worktrees();
    assert!(named(&list, "feat/try").removable());
    assert!(named(&list, "old/thing").removable());
    let leftovers = serde_json::from_str::<List>(fixture!("worktrees/leftovers.json")).expect("the list parses");
    assert!(leftovers.worktrees[0].removable());
}

#[test]
fn a_role_of_a_running_flow_and_a_row_being_removed_cannot_be_ticked() {
    assert!(!Worktree { run_role: true, ..Worktree::default() }.removable());
    assert!(!Worktree { removing: true, ..Worktree::default() }.removable());
}

#[test]
fn a_row_with_no_branch_goes_by_its_folder() {
    let leftover = Worktree {
        project_name: "shop".into(),
        path: "/home/ana/.cornercase/worktrees/shop/gone-repo".into(),
        ..Worktree::default()
    };
    assert_eq!(leftover.title(), "shop › gone-repo");
    assert_eq!(last_folder("C:\\Users\\ana\\gone"), "gone");
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
    let now = epoch("2026-10-29T21:00:00.000Z").expect("a time");
    assert_eq!(totals(&list(), false, now), ["316.0 KB+ in 3 worktrees"]);
}

#[test]
fn the_total_names_builds_left_idle_for_a_week() {
    let later = epoch("2026-11-10T00:00:00.000Z").expect("a time");
    assert_eq!(totals(&list(), false, later)[1], "296.0 KB of builds idle 7+ days");
}

#[test]
fn the_list_is_asked_for_json() {
    assert_eq!(list_command(false, None), ["worktrees", "list", "--json", "--no-sizes"]);
}

#[test]
fn one_project_is_listed_by_its_id() {
    assert_eq!(list_command(true, Some(7)), ["worktrees", "list", "--json", "--project", "7"]);
    assert_eq!(list_command(false, Some(7)), ["worktrees", "list", "--json", "--no-sizes", "--project", "7"]);
}

#[test]
fn the_sheet_names_the_project_it_shows() {
    assert_eq!(scope_note(Some("shop")), "Only the worktrees of shop.");
    assert_eq!(scope_note(None), "Only the worktrees of this project.");
}

fn merged_list(name: &str) -> List {
    let text = match name {
        "list" => fixture!("worktrees/merged/list.json"),
        "none" => fixture!("worktrees/merged/list-none.json"),
        "one" => fixture!("worktrees/merged/list-one.json"),
        "no-sizes" => fixture!("worktrees/merged/list-no-sizes.json"),
        _ => fixture!("worktrees/merged/list-0.13.69.json"),
    };
    serde_json::from_str(text).expect("the list parses")
}

fn later() -> i64 {
    epoch("2026-11-14T12:00:00.000Z").expect("a time")
}

#[test]
fn the_measured_list_carries_its_merged_builds() {
    assert_eq!(merged_list("list").merged_builds, Some(MergedBuilds { bytes: 14_798_848, count: 4 }));
}

#[test]
fn the_list_without_sizes_has_no_merged_figure() {
    assert_eq!(merged_list("no-sizes").merged_builds, None);
}

#[test]
fn an_older_server_sends_no_merged_figure() {
    let list = merged_list("old");
    assert_eq!(list.merged_builds, None);
    assert_eq!(totals(&list, false, later()), ["11.2 MB in 4 worktrees", "6.0 MB of builds idle 7+ days"]);
}

#[test]
fn the_footer_adds_the_merged_builds_in_the_tuis_words() {
    assert_eq!(
        totals(&merged_list("list"), false, later()),
        ["20.2 MB+ in 6 worktrees", "6.0 MB of builds idle 7+ days", "14.1 MB of builds in 4 merged worktrees"]
    );
}

#[test]
fn one_merged_worktree_is_named_in_the_singular() {
    let parts = totals(&merged_list("one"), false, later());
    assert_eq!(parts.last().map(String::as_str), Some("5.1 MB of builds in 1 merged worktree"));
}

#[test]
fn a_count_of_zero_adds_nothing() {
    assert_eq!(totals(&merged_list("none"), false, later()), ["100.0 KB+ in 6 worktrees"]);
}

#[test]
fn the_merged_part_stands_without_an_idle_one() {
    let mut list = merged_list("list");
    list.worktrees.retain(|w| w.branch.as_deref() != Some("feat/e"));
    let parts = totals(&list, false, later());
    assert_eq!(parts, ["14.2 MB+ in 5 worktrees", "14.1 MB of builds in 4 merged worktrees"]);
}

#[test]
fn nothing_shows_while_the_sheet_measures() {
    let list = merged_list("list");
    assert_eq!(list.merged(true), None);
    assert_eq!(totals(&list, true, later()).len(), 2);
}

#[test]
fn a_removed_row_takes_the_figure_with_it() {
    let mut list = merged_list("list");
    list.drop_row("/private/tmp/cc255/worktrees/shop/feat-a");
    assert_eq!(list.worktrees.len(), 5);
    assert_eq!(list.merged(false), None);
}

#[test]
fn the_plus_stays_on_the_first_figure() {
    let parts = totals(&merged_list("list"), false, later());
    assert!(parts[0].contains("MB+ in"));
    assert!(parts[1..].iter().all(|part| !part.contains('+') || part.ends_with("7+ days")));
}

#[test]
fn a_worktree_missing_from_the_list_goes_by_its_folder() {
    let list = merged_list("list");
    assert_eq!(name_of(&list.worktrees, "/private/tmp/cc255/worktrees/shop/gone"), "gone");
    assert_eq!(name_of(&list.worktrees, "/private/tmp/cc255/worktrees/shop/feat-a"), "shop › feat/a");
}

#[test]
fn reading_again_forgets_the_figure_and_keeps_the_rows() {
    let mut list = merged_list("list");
    list.forget_merged();
    assert_eq!(list.merged(false), None);
    assert_eq!(list.worktrees.len(), 6);
}
