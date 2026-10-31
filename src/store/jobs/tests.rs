use std::time::Instant;

use super::{Busy, Jobs, SHOW_AFTER};

fn later() -> Instant {
    Instant::now() + SHOW_AFTER
}

#[test]
fn a_job_is_busy_until_it_ends() {
    let mut jobs = Jobs::default();
    let id = jobs.start(Busy::new("free:/tmp/a", "Freeing builds…"));
    assert!(jobs.busy("free:/tmp/a"));
    jobs.end(id);
    assert!(!jobs.busy("free:/tmp/a"));
}

#[test]
fn ending_one_job_keeps_another_with_the_same_key() {
    let mut jobs = Jobs::default();
    let first = jobs.start(Busy::new("plugin:git", "Updating git…"));
    jobs.start(Busy::new("plugin:git", "Updating git…"));
    jobs.end(first);
    assert!(jobs.busy("plugin:git"));
}

#[test]
fn the_summary_names_the_newest_job_and_counts_the_rest() {
    let mut jobs = Jobs::default();
    assert_eq!(jobs.summary(later()), None);
    jobs.start(Busy::new("a", "Installing 0.13…"));
    assert_eq!(jobs.summary(later()).as_deref(), Some("Installing 0.13…"));
    jobs.start(Busy::remove_worktree(4, "feat/x"));
    assert_eq!(jobs.summary(later()).as_deref(), Some("Removing feat/x… and 1 more"));
}

#[test]
fn a_quick_job_never_reaches_the_summary() {
    let mut jobs = Jobs::default();
    jobs.start(Busy::new("close-tab:3", "Closing the tab…"));
    assert_eq!(jobs.summary(Instant::now()), None);
}

#[test]
fn a_worktree_is_removing_while_its_job_runs() {
    let mut jobs = Jobs::default();
    let id = jobs.start(Busy::remove_worktree(4, "feat/x"));
    assert!(jobs.removing(4));
    assert!(!jobs.removing(5));
    jobs.end(id);
    assert!(!jobs.removing(4));
}

#[test]
fn a_removed_workspace_stays_removing_until_the_session_drops_it() {
    let mut jobs = Jobs::default();
    let id = jobs.start(Busy::remove_worktree(4, "feat/x"));
    jobs.end(id);
    jobs.removed(4);
    jobs.keep_leaving(|workspace| workspace == 4);
    assert!(jobs.removing(4));
    jobs.keep_leaving(|_| false);
    assert!(!jobs.removing(4));
}

#[test]
fn a_running_job_says_what_it_does_under_its_key() {
    let mut jobs = Jobs::default();
    jobs.start(Busy::new("changes-file:3:a.rs", "Staging a.rs…"));
    assert_eq!(jobs.doing("changes-file:3:a.rs"), Some("Staging a.rs…"));
    assert_eq!(jobs.doing("changes-file:3:b.rs"), None);
}
