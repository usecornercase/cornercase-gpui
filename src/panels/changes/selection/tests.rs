use super::super::wire::Mode;
use super::*;
use crate::test_util::vps_aim;

fn selected(workspace: Whose) -> Selected {
    Selected { workspace, path: "src/a.rs".into(), mode: Mode::Commits }
}

#[test]
fn a_machines_file_is_asked_with_the_machines_own_id() {
    assert_eq!(
        file_command("file", &selected(Whose::There(vps_aim(), "ana@vps.example.com".into())), &[]),
        ["changes", "file", "src/a.rs", "--workspace", "7", "--mode", "commits", "--json"]
    );
}

#[test]
fn a_machines_gap_is_asked_with_the_machines_own_id() {
    assert_eq!(
        file_command("gap", &selected(Whose::There(vps_aim(), "ana@vps.example.com".into())), &args(&["--hunk", "1"])),
        ["changes", "gap", "src/a.rs", "--hunk", "1", "--workspace", "7", "--mode", "commits", "--json"]
    );
}

#[test]
fn a_file_here_is_asked_as_it_always_was() {
    assert_eq!(
        file_command("file", &selected(Whose::Here(43)), &[]),
        ["changes", "file", "src/a.rs", "--workspace", "43", "--mode", "commits", "--json"]
    );
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
