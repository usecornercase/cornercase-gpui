use super::super::wire::Mode;
use super::*;
use crate::cli;
use crate::store::Reach;
use crate::test_util::{reads_with_boot, vps_aim, vps_read, vps_target};

fn selected(workspace: Whose) -> Selected {
    Selected { workspace, path: "src/a.rs".into(), mode: Mode::Commits }
}

fn on_a_machine() -> Selected {
    selected(Whose::There(vps_aim(), "ana@vps.example.com".into()))
}

#[test]
fn a_machines_file_is_asked_with_the_machines_own_id() {
    assert_eq!(
        file_command("file", &on_a_machine(), &[], ReadReleases::default()),
        ["changes", "file", "src/a.rs", "--workspace", "7", "--mode", "commits", "--json"]
    );
}

#[test]
fn a_machines_gap_is_asked_with_the_machines_own_id() {
    let hunk = args(&["--hunk", "1"]);
    assert_eq!(
        file_command("gap", &on_a_machine(), &hunk, ReadReleases::default()),
        ["changes", "gap", "src/a.rs", "--hunk", "1", "--workspace", "7", "--mode", "commits", "--json"]
    );
}

#[test]
fn a_file_here_is_asked_as_it_always_was() {
    assert_eq!(
        file_command("file", &selected(Whose::Here(43)), &[], reads_with_boot()),
        ["changes", "file", "src/a.rs", "--workspace", "43", "--mode", "commits", "--json"]
    );
}

#[test]
fn a_file_read_on_a_machine_carries_the_boot_of_its_aim_reads_with_boot() {
    let boot = vps_aim().boot;
    let file = file_command("file", &on_a_machine(), &[], reads_with_boot());
    assert_eq!(file[file.len() - 3..], ["--json", "--boot", boot.as_str()]);
    let hunk = args(&["--hunk", "1"]);
    let gap = file_command("gap", &on_a_machine(), &hunk, reads_with_boot());
    assert_eq!(gap[gap.len() - 3..], ["--json", "--boot", boot.as_str()]);
}

#[test]
fn a_refused_file_read_says_the_machine_restarted_and_any_other_failure_keeps_its_words() {
    let refused = cli::failure("changes", Some(4), b"stale");
    assert_eq!(refusal_text(Some("vps"), &refused), "vps restarted");
    let other = cli::failure("changes", Some(1), b"no such file");
    assert_eq!(refusal_text(Some("vps"), &other), other.to_string());
    assert_eq!(refusal_text(None, &refused), refused.to_string());
}

fn opened(digest: &str) -> Open {
    Open { digest: digest.into(), loading: true, ..Open::default() }
}

#[test]
fn a_patch_that_failed_is_asked_again_by_the_next_summary() {
    let mut open = opened("d5dccd1bdadf10a2");
    open.read_failed("ssh: connection refused".into());
    assert_ne!(open.digest, "d5dccd1bdadf10a2");
    assert_eq!(open.error.as_deref(), Some("ssh: connection refused"));
}

#[test]
fn a_patch_that_failed_with_nothing_on_screen_says_so_in_its_place() {
    assert!(!opened("d5dccd1bdadf10a2").read_failed("no".into()));
}

#[test]
fn reads_under_way_are_not_believed_after_a_restart_and_the_file_is_read_again() {
    let mut open = opened("d5dccd1bdadf10a2");
    let asked = open.generation;
    open.drop_reads_under_way();
    assert_ne!(open.generation, asked);
    assert!(!open.loading);
    assert_ne!(open.digest, "d5dccd1bdadf10a2");
}

#[test]
fn a_failed_file_or_gap_read_latches_a_refusal_and_words_it() {
    let mut hearing = Hearing::default();
    hearing.believed();
    let refused = cli::failure("changes", Some(4), b"error: another run");
    assert_eq!(failed_read(&mut hearing, Some("vps"), &refused), "vps restarted");
    let read = vps_read(Reach::Up(vps_target()));
    assert_eq!(hearing.may_read(&read), Err("vps restarted".to_string()));
}

#[test]
fn any_other_failed_read_keeps_its_words_and_latches_nothing() {
    let mut hearing = Hearing::default();
    hearing.believed();
    let failed = cli::failure("changes", Some(1), b"no such file");
    assert_eq!(failed_read(&mut hearing, Some("vps"), &failed), failed.to_string());
    let read = vps_read(Reach::Up(vps_target()));
    assert!(hearing.may_read(&read).is_ok());
}
