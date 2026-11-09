use super::*;

fn release(version: &str) -> Release {
    Release { version: version.into(), notes: Vec::new() }
}

fn offered(version: &str, installed: bool) -> Update {
    Update { release: Some(release(version)), installed, ..Update::default() }
}

fn disk(text: &str) -> Option<Version> {
    Version::parse(text)
}

#[test]
fn the_chip_names_cornercase() {
    let found = Chip::of(false, None, Some(&release("0.13.19")), &InstallState::Idle).expect("a chip");
    assert_eq!(found.label(), "↑ cornercase 0.13.19");
    assert_eq!(Chip::Installing("0.13.19".into()).label(), "installing cornercase 0.13.19…");
    assert_eq!(Chip::Restart("0.13.19".into()).label(), "↻ restart cornercase for 0.13.19");
    assert_eq!(Chip::Pending.label(), "↻ cornercase restart pending");
}

#[test]
fn there_is_no_chip_without_a_release() {
    assert_eq!(Chip::of(false, None, None, &InstallState::Idle), None);
}

#[test]
fn the_chip_keeps_installing_while_the_update_runs() {
    let out = release("0.13.19");
    let seen = Chip::of(false, Some("0.13.19".into()), Some(&out), &InstallState::Running);
    assert_eq!(seen, Some(Chip::Installing("0.13.19".into())));
}

#[test]
fn the_chip_follows_the_install() {
    let out = release("0.13.19");
    let running = Chip::of(false, None, Some(&out), &InstallState::Running);
    assert_eq!(running, Some(Chip::Installing("0.13.19".into())));
    let installed = Chip::of(false, Some("0.13.19".into()), Some(&out), &InstallState::Idle);
    assert_eq!(installed, Some(Chip::Restart("0.13.19".into())));
    let pending = Chip::of(true, Some("0.13.19".into()), Some(&out), &InstallState::Idle);
    assert_eq!(pending, Some(Chip::Pending));
}

#[test]
fn an_install_by_the_server_or_this_window_offers_the_restart() {
    let done = InstallState::Done("0.13.19".into());
    assert_eq!(installed(&done, None, None).as_deref(), Some("0.13.19"));
    let update = offered("0.13.19", true);
    assert_eq!(installed(&InstallState::Idle, Some(&update), None).as_deref(), Some("0.13.19"));
}

#[test]
fn a_cornercase_already_on_disk_offers_the_restart() {
    let update = offered("0.13.19", false);
    assert_eq!(installed(&InstallState::Idle, Some(&update), disk("0.13.19")).as_deref(), Some("0.13.19"));
    assert_eq!(installed(&InstallState::Idle, Some(&update), disk("0.13.20")).as_deref(), Some("0.13.20"));
}

#[test]
fn an_older_cornercase_on_disk_still_offers_the_release() {
    let update = offered("0.13.19", false);
    assert_eq!(installed(&InstallState::Idle, Some(&update), disk("0.13.1")), None);
    assert_eq!(installed(&InstallState::Idle, Some(&update), None), None);
    assert_eq!(installed(&InstallState::Idle, None, disk("0.13.19")), None);
}
