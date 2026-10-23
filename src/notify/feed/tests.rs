use super::*;

fn fixture(name: &str) -> Event {
    let text = match name {
        "notice" => crate::test_util::fixture!("events/notice.json"),
        "notice-sound" => crate::test_util::fixture!("events/notice-sound.json"),
        "clipboard" => crate::test_util::fixture!("events/clipboard.json"),
        "bell" => crate::test_util::fixture!("events/bell.json"),
        "status" => crate::test_util::fixture!("events/status.json"),
        "dropped" => crate::test_util::fixture!("events/dropped.json"),
        "opened" => crate::test_util::fixture!("events/opened-accounts.json"),
        _ => unreachable!(),
    };
    parse_event(text).expect("a fixture parses")
}

#[test]
fn reads_a_notice() {
    let Event::Notice(n) = fixture("notice") else { panic!("not a notice") };
    assert_eq!(n.token, "cc-ab12cd-3");
    assert_eq!(n.message, "claude needs you in shop › default");
    assert_eq!(n.panes, [4]);
    assert_eq!(n.sound, None);
    assert!(n.toast_kind() == ToastKind::NeedsYou);
    assert_eq!(n.headline(), "claude needs you");
}

#[test]
fn reads_a_notice_with_its_sound() {
    let Event::Notice(n) = fixture("notice-sound") else { panic!("not a notice") };
    assert_eq!(n.sound.as_deref(), Some("finished"));
    assert!(n.toast_kind() == ToastKind::Done);
    assert_eq!(n.headline(), "claude finished");
}

#[test]
fn reads_host_events() {
    assert_eq!(fixture("clipboard"), Event::Clipboard { text: "hello\nworld".into(), pane: Some(4) });
    assert_eq!(fixture("bell"), Event::Bell { count: 3, tab: Some(3), pane: Some(4) });
}

#[test]
fn other_events_are_skipped() {
    assert_eq!(fixture("status"), Event::Other);
    assert_eq!(fixture("dropped"), Event::Other);
    assert_eq!(fixture("opened"), Event::Other);
    assert_eq!(parse_event(r#"{"event":"something-new","x":1}"#), Some(Event::Other));
    assert_eq!(parse_event("pane 5  claude  working → waiting"), None);
}

fn snapshot() -> AppState {
    let value: Value = serde_json::from_str(crate::test_util::fixture!("app/snapshot.json")).expect("fixture");
    AppState::from_value(&value)
}

#[test]
fn reads_the_app_state() {
    let app = snapshot();
    assert_eq!(app.revision, 4);
    assert_eq!(app.boot.as_deref(), Some("4242-1760600000"));
    let release = app.release().expect("a release");
    assert_eq!(release.version, "9.0.0");
    assert_eq!(release.notes.len(), 2);
    assert_eq!(release.notes[1].text, "- Skipped one.");
    let update = app.update.as_ref().expect("update");
    assert_eq!(update.install.as_ref().map(|i| i.method.as_str()), Some("replace"));
    assert!(!update.installed);
}

#[test]
fn reads_away_and_keep_awake_from_the_app_state() {
    let app = snapshot();
    assert!(app.away_ready());
    assert_eq!(app.away_on(), ["telegram"]);
    let awake = app.keep_awake.as_ref().expect("keep awake");
    assert!(awake.held);
    assert_eq!(awake.by.as_deref(), Some("caffeinate"));
}

#[test]
fn reads_an_app_state_without_an_update() {
    let value = serde_json::json!({
        "away": [{"away": false, "platform": "telegram", "ready": false}],
        "keep_awake": null,
        "update": {"checked": "2026-10-23T17:35:31.598Z", "checking": false, "error": null,
            "install": {"command": "brew update && brew upgrade cornercase", "method": "command"},
            "install_error": null, "installed": false, "installing": false, "none": "up-to-date",
            "release": null},
        "version": "0.12.189"
    });
    let app = AppState::from_value(&value);
    assert!(app.release().is_none());
    assert!(!app.away_ready());
    assert!(app.keep_awake.is_none());
    let install = app.update.and_then(|u| u.install).expect("install");
    assert_eq!(install.command.as_deref(), Some("brew update && brew upgrade cornercase"));
}

#[test]
fn reads_the_prefs() {
    let config = serde_json::json!({"settings": [
        {"key": "desktop_notifications", "value": "off"},
        {"key": "sound", "value": true},
        {"key": "sound_waiting", "value": "~/ding.wav"},
        {"key": "sound_done", "value": ""},
    ]});
    let prefs = Prefs::from_config(&config);
    assert!(!prefs.desktop);
    assert_eq!(prefs.sound_waiting, "~/ding.wav");
    assert!(Prefs::from_config(&serde_json::json!({})).desktop);
}

#[test]
fn a_sound_file_that_is_not_there_plays_the_system_sound() {
    let prefs = Prefs { sound_waiting: "~/ding.wav".into(), ..Prefs::default() };
    assert_eq!(prefs.sound_file("needs-you", "/nowhere"), system_sound("needs-you"));
    assert_eq!(prefs.sound_file("finished", "/nowhere"), system_sound("finished"));
}

#[test]
fn plays_the_users_own_file() {
    let dir = std::env::temp_dir().join(format!("cornercase-gpui-sound-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");
    std::fs::write(dir.join("ding.wav"), b"RIFF").expect("file");
    let prefs = Prefs { sound_waiting: "~/ding.wav".into(), ..Prefs::default() };
    let home = dir.to_string_lossy().to_string();
    assert_eq!(prefs.sound_file("needs-you", &home), format!("{home}/ding.wav"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_relative_sound_path_plays_the_system_sound() {
    let prefs = Prefs { sound_waiting: "ding.wav".into(), ..Prefs::default() };
    assert_eq!(prefs.sound_file("needs-you", "/tmp"), system_sound("needs-you"));
}
