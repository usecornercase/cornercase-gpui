use super::{
    Asked, CHECKING_MERGED, FREE_MERGED_SINCE, FREEING_MERGED, FreeMerged, Freed, Merged, MergedJob, ask_merged,
    free_command, free_merged, free_merged_command, free_message, free_title, merged_job, merged_toast, row_toast,
};
use crate::cli::Version;
use crate::desktop;
use crate::panels::poll::parse_text;
use crate::panels::worktrees::data::{List, MergedBuilds, Worktree};
use crate::panels::worktrees::removal::Style;
use crate::store::ToastKind;
use crate::test_util::fixture;

fn rows() -> Vec<Worktree> {
    serde_json::from_str::<List>(fixture!("worktrees/merged/list.json")).expect("the list parses").worktrees
}

fn check() -> Merged {
    serde_json::from_str(fixture!("worktrees/merged/check.json")).expect("the check parses")
}

fn freed() -> Merged {
    serde_json::from_str(fixture!("worktrees/merged/freed.json")).expect("the free parses")
}

fn all_kept() -> Merged {
    serde_json::from_str(fixture!("worktrees/merged/all-kept.json")).expect("the check parses")
}

fn question(answer: &Merged, filtering: bool) -> (String, Vec<(String, Style)>, String) {
    match ask_merged(answer, &rows(), filtering) {
        Asked::Question { title, lines, job } => (title, lines, format!("{job:?}")),
        other => panic!("expected a question, got {other:?}"),
    }
}

fn texts(lines: &[(String, Style)]) -> Vec<&str> {
    lines.iter().map(|(text, _)| text.as_str()).collect()
}

fn with_outcome(mut answer: Merged, index: usize, outcome: &str) -> Merged {
    answer.worktrees[index].outcome = outcome.to_string();
    answer
}

const A: &str = "shop › feat/a";
const NOTE_SEVERAL: &str = "The worktrees, their branches and their changes stay; files inside these folders are lost, \
                            and each project's setup can rebuild them.";
const NOTE_ONE: &str = "The worktree, its branch and its changes stay; files inside these folders are lost, \
                        and the project's setup can rebuild them.";
const HINT: &str = "Where a program runs, Free builds… on that worktree's row can free it anyway.";

#[test]
fn the_merged_free_asks_for_json_and_never_forces() {
    assert_eq!(free_merged_command(false, None), ["worktrees", "free-builds", "--merged", "--json"]);
}

#[test]
fn the_merged_check_deletes_nothing() {
    assert_eq!(free_merged_command(true, None), ["worktrees", "free-builds", "--merged", "--json", "--check"]);
}

#[test]
fn one_projects_sheet_frees_only_that_project() {
    assert_eq!(
        free_merged_command(false, Some(7)),
        ["worktrees", "free-builds", "--merged", "--json", "--project", "7"]
    );
    assert_eq!(
        free_merged_command(true, Some(7)),
        ["worktrees", "free-builds", "--merged", "--json", "--check", "--project", "7"]
    );
}

#[test]
fn a_rows_commands_ask_for_json() {
    assert_eq!(free_command("/w", true, false), ["worktrees", "free-builds", "--path", "/w", "--json", "--check"]);
    assert_eq!(free_command("/w", false, true), ["worktrees", "free-builds", "--path", "/w", "--json", "--force"]);
}

#[test]
fn freeing_warns_about_what_runs_in_the_worktree() {
    let freed: Freed = serde_json::from_str(fixture!("worktrees/free-check.json")).expect("the check parses");
    let text = free_message(&freed);
    assert!(text.starts_with("Frees 296.0 KB: node_modules 296.0 KB. The worktree, its branch"));
    assert!(text.ends_with("Python runs in it and may still use them."));
}

#[test]
fn the_check_reads_every_outcome() {
    let (title, lines, _) = question(&check(), false);
    assert_eq!(title, "Free 5.0 MB of builds in 2 merged worktrees?");
    assert_eq!(lines.iter().filter(|(_, style)| *style == Style::Heading).count(), 2);
    assert_eq!(lines.iter().filter(|(_, style)| *style == Style::Risky).count(), 2);
}

#[test]
fn one_worktree_is_asked_about_by_its_name() {
    let mut answer = check();
    answer.worktrees.remove(1);
    let (title, _, job) = question(&answer, false);
    assert_eq!(title, free_title(A));
    assert!(job.contains("Freeing the builds of shop › feat/a…"));
}

#[test]
fn several_worktrees_are_freed_by_a_job_that_counts_them() {
    let (_, _, job) = question(&check(), false);
    assert!(job.contains(FREEING_MERGED) && job.contains("Freeing the builds of 2 merged worktrees…"));
}

#[test]
fn each_worktree_that_goes_names_its_folders() {
    let (_, lines, _) = question(&check(), false);
    assert_eq!(lines[0], (A.to_string(), Style::Heading));
    assert_eq!(lines[1], ("frees 2.0 MB: target 2.0 MB".to_string(), Style::Line));
}

#[test]
fn a_kept_worktree_is_named_with_why() {
    let (_, lines, _) = question(&check(), false);
    assert_eq!(lines[4], ("shop › feat/c".to_string(), Style::Risky));
    assert_eq!(lines[5], ("keeps its builds: sleep runs in it".to_string(), Style::Lost));
    assert_eq!(lines[7], ("keeps its builds: target/vendor/dep is a git checkout".to_string(), Style::Lost));
}

#[test]
fn a_running_program_points_to_its_rows_free() {
    let (_, lines, _) = question(&check(), false);
    assert!(texts(&lines).contains(&HINT));
}

#[test]
fn a_git_checkout_alone_gets_no_hint() {
    let mut answer = check();
    answer.worktrees.remove(2);
    let (_, lines, _) = question(&answer, false);
    assert!(!texts(&lines).contains(&HINT));
}

#[test]
fn the_question_says_the_filter_does_not_narrow_it() {
    let note = "This frees every worktree named here, including those the filter hides.";
    assert!(texts(&question(&check(), true).1).contains(&note));
    assert!(!texts(&question(&check(), false).1).contains(&note));
}

#[test]
fn several_worktrees_say_what_stays_in_the_plural() {
    assert!(texts(&question(&check(), false).1).contains(&NOTE_SEVERAL));
}

#[test]
fn one_worktree_says_it_in_the_singular() {
    let mut answer = check();
    answer.worktrees.remove(1);
    assert!(texts(&question(&answer, false).1).contains(&NOTE_ONE));
}

#[test]
fn twelve_worktrees_are_all_listed() {
    let mut answer = check();
    let first = answer.worktrees[0].clone();
    answer.worktrees = (0..12)
        .map(|i| {
            let mut entry = first.clone();
            entry.freed.path = format!("/private/tmp/cc255/worktrees/shop/feat-{i}");
            entry
        })
        .collect();
    answer.bytes = 12 * first.freed.bytes;
    let (title, lines, _) = question(&answer, false);
    assert_eq!(title, "Free 24.0 MB of builds in 12 merged worktrees?");
    assert_eq!(lines.iter().filter(|(_, style)| *style == Style::Heading).count(), 12);
    assert_eq!(lines.len(), 25);
}

#[test]
fn when_nothing_can_go_nothing_is_asked() {
    assert_eq!(
        ask_merged(&all_kept(), &rows(), false),
        Asked::Cannot("Cannot free shop › feat/d: target/vendor/dep is a git checkout".to_string())
    );
}

#[test]
fn when_nothing_can_go_a_running_program_is_still_pointed_out() {
    let mut answer = check();
    answer.worktrees.drain(..2);
    let Asked::Cannot(text) = ask_merged(&answer, &rows(), false) else { panic!("nothing goes") };
    assert!(text.ends_with(HINT));
}

#[test]
fn an_empty_check_has_nothing_to_free() {
    assert_eq!(ask_merged(&Merged::default(), &rows(), false), Asked::Nothing);
    assert_eq!(
        merged_toast(&Merged::default(), &rows()),
        ("No merged worktree has builds to free".into(), ToastKind::Info)
    );
}

#[test]
fn a_failed_measure_cannot_go() {
    let answer = with_outcome(check(), 0, "failed");
    let (title, lines, _) = question(&answer, false);
    assert_eq!(title, free_title("shop › feat/b"));
    assert!(texts(&lines).contains(&"could not be measured: cornercase did not say why"));
}

#[test]
fn an_unknown_outcome_is_never_counted_as_freed() {
    let answer = with_outcome(freed(), 0, "skipped");
    let (text, kind) = merged_toast(&answer, &rows());
    assert!(
        text.starts_with("Freed 5.0 MB in 1 merged worktree\nCould not free shop › feat/a: cornercase did not say why")
    );
    assert_eq!(kind, ToastKind::Error);
}

#[test]
fn a_missing_why_says_so() {
    let answer = with_outcome(all_kept(), 0, "refused");
    let mut answer = answer;
    answer.worktrees[0].why = None;
    assert_eq!(
        ask_merged(&answer, &rows(), false),
        Asked::Cannot("Cannot free shop › feat/d: cornercase did not say why".to_string())
    );
}

#[test]
fn a_free_that_went_well_is_plain_news() {
    let mut answer = freed();
    answer.worktrees.truncate(2);
    assert_eq!(merged_toast(&answer, &rows()), ("Freed 5.0 MB in 2 merged worktrees".to_string(), ToastKind::Info));
}

#[test]
fn a_kept_worktree_makes_the_toast_an_error_and_says_why() {
    let (text, kind) = merged_toast(&freed(), &rows());
    assert_eq!(
        text,
        "Freed 5.0 MB in 2 merged worktrees\nDid not free shop › feat/c: sleep runs in it\n\
         Did not free shop › feat/d: target/vendor/dep is a git checkout"
    );
    assert_eq!(kind, ToastKind::Error);
}

#[test]
fn a_failed_worktree_is_named_after_the_good_news() {
    let (text, _) = merged_toast(&with_outcome(freed(), 1, "failed"), &rows());
    assert!(
        text.starts_with("Freed 5.0 MB in 1 merged worktree\nCould not free shop › feat/b: cornercase did not say why")
    );
}

#[test]
fn nothing_freed_says_so_first() {
    let mut answer = freed();
    answer.worktrees.drain(..2);
    answer.bytes = 0;
    let (text, kind) = merged_toast(&answer, &rows());
    assert!(text.starts_with("Nothing was freed\nDid not free shop › feat/c: sleep runs in it"));
    assert_eq!(kind, ToastKind::Error);
}

#[test]
fn a_folder_that_could_not_be_deleted_is_named() {
    let mut answer = freed();
    answer.worktrees.truncate(1);
    answer.worktrees[0]
        .freed
        .failed
        .push(super::FailedFolder { path: "target".into(), error: "permission denied".into() });
    let (text, kind) = merged_toast(&answer, &rows());
    assert!(text.ends_with("Could not free target in shop › feat/a: permission denied"));
    assert_eq!(kind, ToastKind::Error);
}

#[test]
fn a_kept_folder_is_named() {
    let mut answer = freed();
    answer.worktrees.truncate(1);
    answer.worktrees[0].freed.kept.push("target".into());
    let (text, _) = merged_toast(&answer, &rows());
    assert!(text.ends_with(
        "Kept target in shop › feat/a: it is no longer an ignored folder there, or it is on another filesystem"
    ));
}

#[test]
fn a_partial_answer_with_its_output_is_read() {
    let answer = parse_text::<Merged>(Ok(fixture!("worktrees/merged/check.json").to_string()));
    assert_eq!(answer.expect("an answer").worktrees.len(), 4);
}

#[test]
fn a_partial_answer_without_output_is_an_error() {
    assert!(parse_text::<Merged>(Ok(String::new())).is_err());
}

#[test]
fn a_rows_free_names_a_folder_it_could_not_delete() {
    let mut row: Freed = serde_json::from_str(fixture!("worktrees/free-check.json")).expect("the check parses");
    assert_eq!(row_toast(&row, A), ("Freed 296.0 KB".to_string(), ToastKind::Info));
    row.failed.push(super::FailedFolder { path: "node_modules".into(), error: "busy".into() });
    assert_eq!(
        row_toast(&row, A),
        ("Freed 296.0 KB\nCould not free node_modules in shop › feat/a: busy".to_string(), ToastKind::Error)
    );
}

fn merged(count: usize) -> MergedBuilds {
    MergedBuilds { bytes: 1, count }
}

fn newest() -> Version {
    Version::new(0, 13, 72)
}

#[test]
fn the_button_shows_with_the_figure() {
    assert_eq!(free_merged(Some(merged(4)), MergedJob::Idle, false, Some(newest())), FreeMerged::Offered);
}

#[test]
fn no_figure_no_button() {
    assert_eq!(free_merged(None, MergedJob::Idle, false, Some(newest())), FreeMerged::Hidden);
}

#[test]
fn the_button_rests_while_a_row_is_busy() {
    assert_eq!(free_merged(Some(merged(4)), MergedJob::Idle, true, Some(newest())), FreeMerged::Rests);
}

#[test]
fn the_button_shows_its_own_check_and_free() {
    assert_eq!(free_merged(None, MergedJob::Checking, true, None), FreeMerged::Checking);
    assert_eq!(free_merged(None, MergedJob::Freeing, true, None), FreeMerged::Freeing);
}

#[test]
fn an_older_command_line_here_gets_a_note_not_a_button() {
    let here = Some(Version::new(0, 13, 69));
    let note = format!(
        "Freeing the merged worktrees' builds in one go needs cornercase 0.13.70 or newer on this {} (it has 0.13.69).",
        desktop::here()
    );
    assert_eq!(free_merged(Some(merged(4)), MergedJob::Idle, false, here), FreeMerged::Needs(note));
    assert_eq!(free_merged(Some(merged(4)), MergedJob::Idle, false, Some(FREE_MERGED_SINCE)), FreeMerged::Offered);
}

#[test]
fn an_unread_command_line_gets_the_note_without_a_release() {
    let note = format!(
        "Freeing the merged worktrees' builds in one go needs cornercase 0.13.70 or newer on this {}.",
        desktop::here()
    );
    assert_eq!(free_merged(Some(merged(4)), MergedJob::Idle, false, None), FreeMerged::Needs(note));
}

#[test]
fn the_merged_jobs_have_keys_of_their_own() {
    assert_eq!(merged_job(|key| key == FREEING_MERGED), MergedJob::Freeing);
    assert_eq!(merged_job(|key| key == CHECKING_MERGED), MergedJob::Checking);
    assert_eq!(merged_job(|key| key == "free-builds:/w"), MergedJob::Idle);
}
