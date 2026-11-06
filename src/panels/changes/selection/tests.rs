use super::super::wire::Mode;
use super::*;
use crate::test_util::vps_aim;

fn selected(workspace: Whose) -> Selected {
    Selected { workspace, path: "src/a.rs".into(), mode: Mode::Commits }
}

#[test]
fn a_machines_file_is_asked_with_the_machines_own_id() {
    assert_eq!(
        file_command("file", &selected(Whose::There(vps_aim())), &[]),
        ["changes", "file", "src/a.rs", "--workspace", "7", "--mode", "commits", "--json"]
    );
}

#[test]
fn a_machines_gap_is_asked_with_the_machines_own_id() {
    assert_eq!(
        file_command("gap", &selected(Whose::There(vps_aim())), &args(&["--hunk", "1"])),
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
