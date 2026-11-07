use super::{Answer, Folder, Said, Style, branch_label, button, cannot_go, check_command, said, summary, title};
use crate::store::ToastKind;
use crate::test_util::fixture;

fn checked() -> Vec<Folder> {
    serde_json::from_str::<Answer>(fixture!("worktrees/remove-check.json")).expect("the check parses").folders
}

fn answers() -> Vec<Folder> {
    serde_json::from_str::<Answer>(fixture!("worktrees/remove-answers.json")).expect("the answers parse").folders
}

fn at(folders: &[Folder], end: &str) -> Folder {
    folders.iter().find(|folder| folder.path.ends_with(end)).expect("the folder is answered").clone()
}

fn only(folder: Folder) -> Vec<Folder> {
    vec![folder]
}

fn line(text: &str) -> Said {
    (text.to_string(), Style::Line)
}

#[test]
fn the_check_names_every_folder_and_stops_nothing() {
    assert_eq!(
        check_command(&["/w/a".to_string(), "/w/b".to_string()]),
        ["worktrees", "remove", "--check", "--json", "--path", "/w/a", "/w/b"]
    );
}

#[test]
fn every_kind_of_row_can_go_but_what_the_listing_does_not_know() {
    let folders = checked();
    assert_eq!(folders.iter().filter(|folder| folder.goes()).count(), 8);
    assert!(!at(&folders, "nowhere").goes());
}

#[test]
fn a_clean_folder_gets_no_flag() {
    let folder = at(&checked(), "feat-clean");
    let command = folder.remove_command(false);
    assert_eq!(command[..3], ["worktrees", "remove", "--path"]);
    assert_eq!(command[4..], ["--json"]);
}

#[test]
fn uncommitted_files_take_force() {
    let command = at(&checked(), "feat-dirty").remove_command(false);
    assert!(command.contains(&"--force".to_string()), "{command:?}");
    assert!(!command.contains(&"--unlock".to_string()), "{command:?}");
}

#[test]
fn a_lock_takes_unlock_and_not_force() {
    let command = at(&checked(), "feat-locked").remove_command(false);
    assert!(command.contains(&"--unlock".to_string()), "{command:?}");
    assert!(!command.contains(&"--force".to_string()), "{command:?}");
}

#[test]
fn a_folder_whose_repository_is_gone_takes_force() {
    assert!(at(&checked(), "gone-repo").remove_command(false).contains(&"--force".to_string()));
}

#[test]
fn commits_that_stay_on_their_branch_take_no_force() {
    let command = at(&checked(), "feat-lost").remove_command(false);
    assert!(!command.contains(&"--force".to_string()), "{command:?}");
    assert!(!command.contains(&"--delete-branch".to_string()), "{command:?}");
}

#[test]
fn deleting_the_branch_that_holds_them_takes_force() {
    let command = at(&checked(), "feat-lost").remove_command(true);
    assert!(command.contains(&"--delete-branch".to_string()), "{command:?}");
    assert!(command.contains(&"--force".to_string()), "{command:?}");
}

#[test]
fn deleting_a_branch_with_nothing_to_lose_takes_no_force() {
    let command = at(&checked(), "feat-clean").remove_command(true);
    assert!(command.contains(&"--delete-branch".to_string()), "{command:?}");
    assert!(!command.contains(&"--force".to_string()), "{command:?}");
}

#[test]
fn one_folder_is_asked_about_by_its_name() {
    assert_eq!(title(&only(at(&checked(), "feat-clean"))), "Remove the worktree shop › feat/clean?");
}

#[test]
fn a_leftover_is_asked_about_by_its_folder() {
    assert_eq!(title(&only(at(&checked(), "gone-repo"))), "Remove the worktree shop › gone-repo?");
}

#[test]
fn several_folders_count_only_those_that_can_go() {
    assert_eq!(title(&checked()), "Remove 8 worktrees?");
}

#[test]
fn the_button_says_nothing_more_when_nothing_is_at_stake() {
    assert_eq!(button(&only(at(&checked(), "feat-clean")), false), "Remove");
}

#[test]
fn the_button_says_anyway_after_the_risk_was_named() {
    assert_eq!(button(&only(at(&checked(), "feat-dirty")), false), "Remove anyway");
}

#[test]
fn the_button_says_it_unlocks() {
    assert_eq!(button(&only(at(&checked(), "feat-locked")), false), "Unlock and remove");
}

#[test]
fn the_button_says_both_for_a_lock_beside_a_risk() {
    let folders = vec![at(&checked(), "feat-locked"), at(&checked(), "feat-dirty")];
    assert_eq!(button(&folders, false), "Unlock and remove anyway");
}

#[test]
fn the_button_changes_when_the_box_would_lose_commits() {
    let folders = only(at(&checked(), "feat-lost"));
    assert_eq!(button(&folders, false), "Remove");
    assert_eq!(button(&folders, true), "Remove anyway");
}

#[test]
fn the_box_names_the_one_branch() {
    assert_eq!(branch_label(&only(at(&checked(), "feat-clean"))).as_deref(), Some("Also delete the branch feat/clean"));
}

#[test]
fn the_box_shows_for_a_prunable_entry_with_a_branch() {
    assert!(branch_label(&only(at(&checked(), "feat-pruned"))).is_some());
}

#[test]
fn the_box_speaks_of_several_branches() {
    let folders = vec![at(&checked(), "feat-clean"), at(&checked(), "feat-lost")];
    assert_eq!(branch_label(&folders).as_deref(), Some("Also delete their branches"));
}

#[test]
fn no_box_where_no_branch_could_go() {
    assert_eq!(branch_label(&only(at(&checked(), "gone-repo"))), None);
    assert_eq!(branch_label(&only(at(&checked(), "nowhere"))), None);
}

#[test]
fn the_question_says_what_the_check_says() {
    let words = said(&only(at(&checked(), "feat-dirty")), false);
    assert_eq!(words[0], ("shop › feat/dirty".to_string(), Style::Risky));
    assert_eq!(words[1..3], [line("1 file changed"), line("teardown runs first")]);
}

#[test]
fn a_folder_with_nothing_at_stake_is_not_marked() {
    assert_eq!(said(&only(at(&checked(), "feat-pruned")), false)[0].1, Style::Heading);
}

#[test]
fn the_lost_commits_show_only_with_the_box() {
    let folders = only(at(&checked(), "feat-lost"));
    let lost = ("feat/lost: its commits will be lost".to_string(), Style::Lost);
    assert!(!said(&folders, false).contains(&lost));
    assert!(said(&folders, true).contains(&lost));
    assert_eq!(said(&folders, true)[0].1, Style::Risky);
}

#[test]
fn a_worktree_on_no_branch_says_none_goes_with_the_box() {
    let mut folder = at(&checked(), "feat-clean");
    folder.worktree.as_mut().expect("a row").branch = None;
    let none = line("no branch is checked out, so none is deleted");
    assert!(!said(&only(folder.clone()), false).contains(&none));
    assert!(said(&only(folder), true).contains(&none));
}

#[test]
fn what_cannot_go_says_why_in_the_question() {
    let words = said(&only(at(&checked(), "nowhere")), false);
    assert_eq!(words[0], ("nowhere".to_string(), Style::Risky));
    assert_eq!(
        words[1],
        ("cannot be removed: nothing is there, and no repository lists a worktree there".to_string(), Style::Lost)
    );
}

#[test]
fn an_open_workspace_says_its_tabs_close() {
    let words = said(&only(at(&checked(), "feat-clean")), false);
    assert_eq!(words.last(), Some(&("Its workspace closes, with its tabs.".to_string(), Style::Note)));
}

#[test]
fn a_closed_projects_worktree_has_no_tabs_to_close() {
    let words = said(&only(at(&checked(), "feat-closed")), false);
    assert!(words.iter().all(|(_, style)| *style != Style::Note), "{words:?}");
    assert!(words.contains(&line("teardown runs first")));
}

#[test]
fn when_nothing_can_go_the_toast_says_why_by_name() {
    assert_eq!(
        cannot_go(&only(at(&checked(), "nowhere"))),
        "Cannot remove nowhere: nothing is there, and no repository lists a worktree there"
    );
}

#[test]
fn a_removal_with_nothing_to_say_is_plain_news() {
    let (text, kind) = summary(&only(at(&answers(), "cap-clean")));
    assert_eq!(text, "Removed shop › cap/clean and the branch cap/clean");
    assert!(kind == ToastKind::Info);
}

#[test]
fn several_removals_add_up() {
    let folders = vec![at(&answers(), "cap-clean"), at(&answers(), "cap-ok")];
    assert_eq!(summary(&folders).0, "Removed 2 worktrees and 2 branches");
}

#[test]
fn a_removal_keeps_counting_a_runs_roles() {
    let mut folder = at(&answers(), "cap-clean");
    folder.roles = vec![serde_json::json!({"role": "coder"}), serde_json::json!({"role": "reviewer"})];
    assert_eq!(summary(&only(folder)).0, "Removed 3 worktrees and the branch cap/clean");
}

#[test]
fn a_branch_that_was_kept_is_named() {
    let (text, kind) = summary(&only(at(&answers(), "feat-onmain")));
    assert_eq!(
        text,
        "Removed shop › main\nshop › main: could not delete the branch main: refusing to delete the branch main, the \
         main checkout's or the default"
    );
    assert!(kind == ToastKind::Error);
}

#[test]
fn a_branch_that_moved_since_the_question_is_named() {
    let mut folder = at(&answers(), "cap-clean");
    folder.branches.clear();
    folder.kept = vec!["kept the branch cap/clean: it changed since the confirmation".to_string()];
    assert_eq!(
        summary(&only(folder)).0,
        "Removed shop › cap/clean\nshop › cap/clean: kept the branch cap/clean: it changed since the confirmation"
    );
}

#[test]
fn a_teardown_that_failed_is_named() {
    let (text, kind) = summary(&only(at(&answers(), "cap-bad")));
    assert_eq!(text, "Removed shop › cap/bad\nshop › cap/bad: its teardown failed: exit status 4: broken");
    assert!(kind == ToastKind::Error);
}

#[test]
fn a_folder_git_refused_says_why_by_name() {
    let (text, kind) = summary(&only(at(&answers(), "cap-writes")));
    assert_eq!(
        text,
        "Could not remove shop › cap/writes: '/home/ana/.cornercase/worktrees/shop/cap-writes' contains modified or \
         untracked files, use --force to delete it"
    );
    assert!(kind == ToastKind::Error);
}

#[test]
fn a_folder_that_changed_after_the_question_is_left_alone_and_said_in_words() {
    let (text, kind) = summary(&only(at(&answers(), "feat-dirty")));
    assert_eq!(
        text,
        "Did not remove shop › feat/dirty: it changed since you were asked (1 file changed; teardown runs first). \
         Remove… asks again."
    );
    assert!(!text.contains("--force"), "{text}");
    assert!(kind == ToastKind::Error);
}

#[test]
fn a_refusal_nothing_could_lift_gives_its_reason() {
    assert_eq!(
        summary(&only(at(&answers(), "nowhere"))).0,
        "Did not remove nowhere: nothing is there, and no repository lists a worktree there"
    );
}

#[test]
fn the_good_news_comes_before_the_bad() {
    let (text, kind) = summary(&answers());
    assert!(text.starts_with("Removed 4 worktrees and 2 branches\n"), "{text}");
    assert!(kind == ToastKind::Error);
}

#[test]
fn a_command_that_failed_as_a_whole_still_names_the_folder() {
    let asked = at(&checked(), "feat-clean");
    let (text, kind) = summary(&only(asked.answered(Err("cornercase worktrees: no such command".to_string()))));
    assert_eq!(text, "Could not remove shop › feat/clean: cornercase worktrees: no such command");
    assert!(kind == ToastKind::Error);
}

#[test]
fn an_answer_that_names_no_row_keeps_the_one_asked_about() {
    let asked = at(&checked(), "feat-clean");
    let mut answer = at(&answers(), "nowhere");
    answer.path.clone_from(&asked.path);
    let folder = asked.answered(Ok(Answer { folders: vec![answer] }));
    assert_eq!(folder.title(), "shop › feat/clean");
    assert!(!folder.removed());
}
