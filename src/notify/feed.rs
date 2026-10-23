//! What the notifier reads: lines of `cornercase events --notices --host --json` (events format 1),
//! the app state of `status --app` (update, away, keep awake) and the few settings of
//! `config --json` that say how to tell a notice (desktop notifications, the sound files). Pure
//! parsing and decisions, so they are tested against cornercase's own fixtures.

use serde::Deserialize;
use serde_json::Value;

use crate::store::ToastKind;

/// One line of `events --json`. Events this client does not act on (status, opened, closed…)
/// and those of a newer server read as `Other`.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "event", rename_all = "lowercase")]
pub(crate) enum Event {
    Notice(Notice),
    Clipboard {
        text: String,
        #[serde(default)]
        pane: Option<u64>,
    },
    Bell {
        #[serde(default = "one")]
        count: u64,
        #[serde(default)]
        tab: Option<u64>,
        #[serde(default)]
        pane: Option<u64>,
    },
    #[serde(other)]
    Other,
}

fn one() -> u64 {
    1
}

/// An agent that needs you, failed or finished, as the window's toast says it.
#[derive(Debug, Deserialize, PartialEq, Clone)]
pub(crate) struct Notice {
    /// For `cornercase go TOKEN`.
    pub token: String,
    #[serde(default)]
    pub agent: Option<String>,
    /// `waiting`, `failed` or `done`.
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub place: Option<String>,
    pub message: String,
    #[serde(default)]
    pub panes: Vec<u64>,
    /// The sound the server chose (`needs-you`, `finished`), absent when it chose none.
    #[serde(default)]
    pub sound: Option<String>,
}

impl Notice {
    pub(crate) fn toast_kind(&self) -> ToastKind {
        match self.status.as_deref() {
            Some("done") => ToastKind::Done,
            _ => ToastKind::NeedsYou,
        }
    }

    /// The desktop notification's title.
    pub(crate) fn headline(&self) -> String {
        let agent = self.agent.as_deref().unwrap_or("an agent");
        match self.status.as_deref() {
            Some("done") => format!("{agent} finished"),
            Some("failed") => format!("{agent} failed"),
            _ => format!("{agent} needs you"),
        }
    }
}

pub(super) fn parse_event(line: &str) -> Option<Event> {
    serde_json::from_str(line.trim()).ok()
}

// ---------------------------------------------------------------------------------------------

/// The parts of `status --app` the status bar shows.
#[derive(Debug, Default, Deserialize, PartialEq)]
pub(crate) struct AppState {
    #[serde(default)]
    pub boot: Option<String>,
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub away: Vec<Away>,
    #[serde(default)]
    pub update: Option<Update>,
    #[serde(default)]
    pub keep_awake: Option<KeepAwake>,
}

#[derive(Debug, Default, Deserialize, PartialEq)]
pub(crate) struct Away {
    pub platform: String,
    #[serde(default)]
    pub ready: bool,
    #[serde(default)]
    pub away: bool,
}

#[derive(Debug, Default, Deserialize, PartialEq, Clone)]
pub(crate) struct Update {
    #[serde(default)]
    pub release: Option<Release>,
    #[serde(default)]
    pub checking: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub install: Option<Install>,
    #[serde(default)]
    pub installing: bool,
    #[serde(default)]
    pub installed: bool,
    #[serde(default)]
    pub install_error: Option<String>,
}

#[derive(Debug, Default, Deserialize, PartialEq, Clone)]
pub(crate) struct Release {
    pub version: String,
    /// Every changelog section after the running version, newest first.
    #[serde(default)]
    pub notes: Vec<Notes>,
}

#[derive(Debug, Default, Deserialize, PartialEq, Clone)]
pub(crate) struct Notes {
    pub version: String,
    #[serde(default)]
    pub text: String,
}

/// How this cornercase updates: it replaces its own binary (`replace`), or a command to run
/// (`command`, Homebrew and folders it cannot write).
#[derive(Debug, Default, Deserialize, PartialEq, Clone)]
pub(crate) struct Install {
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub command: Option<String>,
}

#[derive(Debug, Default, Deserialize, PartialEq)]
pub(crate) struct KeepAwake {
    #[serde(default)]
    pub held: bool,
    #[serde(default)]
    pub by: Option<String>,
    #[serde(default)]
    pub unavailable: Option<String>,
}

impl AppState {
    pub(crate) fn from_value(value: &Value) -> Self {
        serde_json::from_value(value.clone()).unwrap_or_default()
    }

    /// The release out there, when there is one and it is not installed yet.
    pub(crate) fn release(&self) -> Option<&Release> {
        self.update.as_ref().and_then(|u| u.release.as_ref())
    }

    /// Whether a bot is paired, so away can be turned on.
    pub(crate) fn away_ready(&self) -> bool {
        self.away.iter().any(|a| a.ready)
    }

    /// The platforms away is on for.
    pub(crate) fn away_on(&self) -> Vec<&str> {
        self.away.iter().filter(|a| a.away).map(|a| a.platform.as_str()).collect()
    }
}

/// A platform's name as people write it.
pub(super) fn platform_name(platform: &str) -> String {
    match platform {
        "telegram" => "Telegram".into(),
        "discord" => "Discord".into(),
        other => other.to_string(),
    }
}

// ---------------------------------------------------------------------------------------------

/// How the user asked to be told (`config --json`'s settings).
#[derive(Debug, PartialEq)]
pub(crate) struct Prefs {
    /// `desktop_notifications` is not `off`.
    pub desktop: bool,
    /// `sound_waiting`, `sound_done`: a file of the user's, empty for the built-in sound.
    pub sound_waiting: String,
    pub sound_done: String,
}

impl Default for Prefs {
    fn default() -> Self {
        Self { desktop: true, sound_waiting: String::new(), sound_done: String::new() }
    }
}

impl Prefs {
    pub(crate) fn from_config(config: &Value) -> Self {
        let mut prefs = Prefs::default();
        let Some(settings) = config.get("settings").and_then(Value::as_array) else { return prefs };
        for setting in settings {
            let value = setting.get("value").and_then(Value::as_str).unwrap_or_default().to_string();
            match setting.get("key").and_then(Value::as_str) {
                Some("desktop_notifications") => prefs.desktop = value != "off",
                Some("sound_waiting") => prefs.sound_waiting = value,
                Some("sound_done") => prefs.sound_done = value,
                _ => {}
            }
        }
        prefs
    }

    /// The file to play for a cue: the user's own when it is a readable file (absolute or `~/`,
    /// as cornercase accepts them), else a macOS system sound, one for each cue.
    pub(crate) fn sound_file(&self, cue: &str, home: &str) -> String {
        let own = if cue == "finished" { &self.sound_done } else { &self.sound_waiting };
        let own = match own.strip_prefix("~/") {
            Some(rest) if !home.is_empty() => format!("{home}/{rest}"),
            _ => own.clone(),
        };
        if own.starts_with('/') && std::path::Path::new(&own).is_file() {
            return own;
        }
        system_sound(cue).to_string()
    }
}

pub(super) fn system_sound(cue: &str) -> &'static str {
    if cue == "finished" { "/System/Library/Sounds/Glass.aiff" } else { "/System/Library/Sounds/Ping.aiff" }
}

#[cfg(test)]
mod tests {
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

    #[test]
    fn reads_the_app_state() {
        let value: Value =
            serde_json::from_str(crate::test_util::fixture!("app/snapshot.json")).expect("fixture");
        let app = AppState::from_value(&value);
        assert_eq!(app.revision, 4);
        assert_eq!(app.boot.as_deref(), Some("4242-1760600000"));
        let release = app.release().expect("a release");
        assert_eq!(release.version, "9.0.0");
        assert_eq!(release.notes.len(), 2);
        assert_eq!(release.notes[1].text, "- Skipped one.");
        let update = app.update.as_ref().expect("update");
        assert_eq!(update.install.as_ref().map(|i| i.method.as_str()), Some("replace"));
        assert!(!update.installed);
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
        // A file that is not there plays the system sound.
        assert_eq!(prefs.sound_file("needs-you", "/nowhere"), system_sound("needs-you"));
        assert_eq!(prefs.sound_file("finished", "/nowhere"), system_sound("finished"));
        assert!(Prefs::from_config(&serde_json::json!({})).desktop);
    }

    #[test]
    fn plays_the_users_own_file() {
        let dir = std::env::temp_dir().join(format!("cornercase-gpui-sound-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("ding.wav"), b"RIFF").expect("file");
        let prefs = Prefs { sound_waiting: "~/ding.wav".into(), ..Prefs::default() };
        let home = dir.to_string_lossy().to_string();
        assert_eq!(prefs.sound_file("needs-you", &home), format!("{home}/ding.wav"));
        // A relative path is refused, as cornercase refuses it.
        let prefs = Prefs { sound_waiting: "ding.wav".into(), ..Prefs::default() };
        assert_eq!(prefs.sound_file("needs-you", &home), system_sound("needs-you"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
