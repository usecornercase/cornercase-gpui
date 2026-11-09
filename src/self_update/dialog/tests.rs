use std::path::PathBuf;

use super::*;
use crate::self_update::release::{Asset, Latest};

fn found(way: Way, on_disk: bool) -> Found {
    let latest = Latest { version: "0.2.0".into(), page: "page".into(), body: String::new(), assets: Vec::new() };
    Found { latest, notes: Vec::new(), way, on_disk, changelog: true }
}

fn asset() -> Asset {
    Asset { name: "a.zip".into(), url: "https://example.test/a.zip".into() }
}

#[test]
fn each_way_says_how_it_updates() {
    let replace = how(&Way::Replace { asset: asset() }, Os::Windows);
    assert!(replace.contains("Restart opens it on the same tab"));
    let bundle = how(&Way::Download { url: "u".into(), why: Why::Bundle }, Os::Mac);
    assert!(bundle.contains("drag cornercase to Applications"));
    let folder = how(&Way::Download { url: "u".into(), why: Why::Folder(PathBuf::from("/opt/apps")) }, Os::Linux);
    assert!(
        folder.contains("cornercase-gpui cannot write to /opt/apps") && folder.contains("in place of cornercase-gpui.")
    );
    let applications =
        how(&Way::Download { url: "u".into(), why: Why::Folder(PathBuf::from("/Applications")) }, Os::Mac);
    assert!(
        applications.contains("cannot write to /Applications")
            && applications.contains("drag cornercase to Applications")
    );
    let none = how(&Way::Download { url: "u".into(), why: Why::NoDownload }, Os::Linux);
    assert!(none.contains("no download for this machine"));
    let command = how(&Way::Command("cargo install x".into()), Os::Linux);
    assert_eq!(command, "This copy was built from source. Update it with:\ncargo install x");
}

#[test]
fn windows_says_to_close_the_window_before_cargo() {
    let command = how(&Way::Command("cargo install x".into()), Os::Windows);
    assert!(command.ends_with("Windows does not let a running program be replaced."));
}

#[test]
fn the_status_follows_the_install() {
    let found = found(Way::Command("c".into()), false);
    let text = |install: &InstallState, error| status(install, &found, error).map(|(text, _)| text);
    assert_eq!(text(&InstallState::Idle, None), None);
    assert_eq!(text(&InstallState::Running, None).as_deref(), Some("Downloading and installing…"));
    assert_eq!(
        text(&InstallState::Done("0.2.0".into()), None).as_deref(),
        Some("cornercase-gpui 0.2.0 is installed. Restart to use it.")
    );
    assert_eq!(text(&InstallState::Failed("no".into()), None).as_deref(), Some("no"));
    assert_eq!(text(&InstallState::Idle, Some("offline")).as_deref(), Some("The last check failed: offline"));
}

#[test]
fn a_copy_already_on_disk_says_it_is_installed() {
    let found = found(Way::Command("c".into()), true);
    let said = status(&InstallState::Idle, &found, None).map(|(text, _)| text);
    assert_eq!(said.as_deref(), Some("cornercase-gpui 0.2.0 is installed. Restart to use it."));
}

#[test]
fn the_button_follows_the_way_and_the_install() {
    let replace = found(Way::Replace { asset: asset() }, false);
    assert_eq!(action(&InstallState::Idle, &replace), Action::Install);
    assert_eq!(action(&InstallState::Running, &replace), Action::Busy);
    let failed = InstallState::Failed("no".into());
    assert_eq!(action(&failed, &replace), Action::Install);
    assert_eq!(fallback(&failed, &replace).as_deref(), Some("https://example.test/a.zip"));
    assert_eq!(fallback(&InstallState::Idle, &replace), None);
    assert_eq!(action(&InstallState::Done("0.2.0".into()), &replace), Action::Restart);
    let download = found(Way::Download { url: "u".into(), why: Why::Bundle }, false);
    assert_eq!(action(&InstallState::Idle, &download), Action::Download("u".into()));
    let command = found(Way::Command("c".into()), false);
    assert_eq!(action(&InstallState::Idle, &command), Action::Copy("c".into()));
    let installed = found(Way::Command("c".into()), true);
    assert_eq!(action(&InstallState::Idle, &installed), Action::Restart);
}
