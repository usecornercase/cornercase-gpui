use super::*;
use crate::panels::changes::wire::{Summary, summary};
use crate::test_util::fixture;

fn listed() -> Summary {
    summary(fixture!("changes/list.json")).expect("the list parses")
}

fn file(path: &str) -> FileInfo {
    listed().workspaces[0].files.iter().find(|f| f.path == path).cloned().expect("the file is listed")
}

fn joined(a: &[String]) -> String {
    a.join(" ")
}

#[test]
fn staging_names_the_version_listed() {
    assert_eq!(
        joined(&stage_command(43, &file("a.txt"))),
        "changes stage a.txt --workspace 43 --digest d5dccd1bdadf10a2"
    );
}

#[test]
fn a_fully_staged_file_is_unstaged_by_its_digest() {
    let staged = FileInfo { staged: Some(Staging::Yes), ..file("a.txt") };
    assert_eq!(joined(&stage_command(43, &staged)), "changes unstage a.txt --workspace 43 --digest d5dccd1bdadf10a2");
}

#[test]
fn discarding_names_the_version_listed() {
    assert_eq!(
        joined(&discard_command(43, &file("c.txt"))),
        "changes discard c.txt --workspace 43 --digest 41043f2d29a13472 --yes"
    );
}

#[test]
fn marking_viewed_names_the_digest_instead_of_the_mode() {
    assert_eq!(
        joined(&viewed_command(43, &file("a.txt"), Mode::Commits)),
        "changes viewed a.txt --workspace 43 --digest d5dccd1bdadf10a2"
    );
}

#[test]
fn clearing_viewed_names_the_mode_without_a_digest() {
    let viewed = FileInfo { viewed: true, ..file("a.txt") };
    assert_eq!(
        joined(&viewed_command(43, &viewed, Mode::All)),
        "changes viewed a.txt --workspace 43 --mode all --clear"
    );
}

#[test]
fn without_a_digest_the_version_there_now_is_named_by_mode() {
    let bare = FileInfo { digest: String::new(), ..file("a.txt") };
    assert_eq!(
        joined(&viewed_command(43, &bare, Mode::Uncommitted)),
        "changes viewed a.txt --workspace 43 --mode uncommitted"
    );
    assert_eq!(joined(&stage_command(43, &bare)), "changes stage a.txt --workspace 43");
}
