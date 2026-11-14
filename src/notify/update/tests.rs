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

fn notes(text: &str) -> Vec<Notes> {
    vec![Notes { version: "0.1.37".into(), text: text.into() }]
}

#[test]
fn the_notes_are_parsed_once_while_they_stay_the_same() {
    let mut parsed = ParsedNotes::default();
    let first = parsed.of(&notes("- **Formatted** notes")).as_ptr();
    let again = parsed.of(&notes("- **Formatted** notes")).as_ptr();
    assert_eq!(first, again);
}

#[test]
fn changed_notes_are_parsed_again() {
    let mut parsed = ParsedNotes::default();
    parsed.of(&notes("- one"));
    assert_eq!(parsed.of(&notes("## Two")), [markdown::parse("## Two")]);
    assert_eq!(parsed.of(&[]), Vec::<Vec<Block>>::new());
}

#[test]
fn a_failed_install_names_the_program_and_opens_its_dialog() {
    let toast = UpdateToast::install_failed("cornercase-gpui", "0.1.65", "no space left", Updating::Window);
    assert_eq!(toast.text, "Could not install cornercase-gpui 0.1.65: no space left");
    assert_eq!((toast.kind, toast.details), (ToastKind::Error, Some(Updating::Window)));
}

fn by_hand() -> ByHand {
    ByHand {
        version: "0.13.53".into(),
        latest: "0.13.74".into(),
        command: "brew update && brew upgrade cornercase".into(),
    }
}

#[test]
fn the_dialog_says_where_cornercase_is_installed() {
    assert_eq!(installed_words("0.13.74", None), "cornercase 0.13.74 is installed. Restart to use it.");
    assert_eq!(
        installed_words("0.13.74", Some("vps")),
        "cornercase 0.13.74 is installed on vps; its server keeps running the release it started with until it restarts."
    );
}

#[test]
fn a_manual_answer_says_how_and_where_to_run_its_command() {
    assert_eq!(
        manual_words(&by_hand(), None),
        "cornercase 0.13.53 cannot update itself: run the command above in a terminal to install 0.13.74."
    );
    assert_eq!(
        manual_words(&by_hand(), Some("vps")),
        "cornercase 0.13.53 on vps cannot update itself: run the command above on vps to install 0.13.74."
    );
}

#[test]
fn the_copy_toast_says_where_to_run_the_command() {
    assert_eq!(copied_words(None), "Copied: run it in a terminal");
    assert_eq!(copied_words(Some("vps")), "Copied: run it on vps");
}

#[test]
fn the_manual_lines_command_is_the_one_offered() {
    let app = Some("curl | sh".to_string());
    let manual = InstallState::Manual(by_hand());
    assert_eq!(offered_command(&manual, app.clone()).as_deref(), Some("brew update && brew upgrade cornercase"));
    assert_eq!(offered_command(&InstallState::Idle, app.clone()), app);
    assert_eq!(offered_command(&InstallState::Idle, None), None);
}

#[test]
fn a_manual_answer_shows_its_line_until_the_release_is_on_disk() {
    let update = Update::default();
    let manual = InstallState::Manual(by_hand());
    let (text, colour) = install_status(&manual, &update, (None, None)).expect("a line");
    assert_eq!(text, manual_words(&by_hand(), None));
    assert_eq!(colour, colours::muted());
    let (text, colour) = install_status(&manual, &update, (Some("0.13.74"), None)).expect("a line");
    assert_eq!(text, "cornercase 0.13.74 is installed. Restart to use it.");
    assert_eq!(colour, colours::done());
}

#[test]
fn a_manual_answer_leaves_the_chip_on_the_release() {
    let out = release("0.13.74");
    let seen = Chip::of(false, None, Some(&out), &InstallState::Manual(by_hand()));
    assert_eq!(seen, Some(Chip::Found("0.13.74".into())));
}

#[test]
fn a_manual_answer_is_not_an_install() {
    let offer = offered("0.13.74", false);
    let manual = InstallState::Manual(by_hand());
    assert_eq!(installed(&manual, Some(&offer), None), None);
    assert_eq!(installed(&manual, Some(&offer), disk("0.13.74")).as_deref(), Some("0.13.74"));
}
