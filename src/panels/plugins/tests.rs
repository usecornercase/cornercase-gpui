use super::data::{List, Manifest, Plugin, Ran, duration};
use crate::store::ToastKind;

#[test]
fn reads_the_plugin_list() {
    let text = r#"{"plugins": [
        {"enabled": true, "error": null, "id": "gpui.test", "log": "/x/plugin.log",
         "manifest": {"actions": [{"command": ["bin/hello"], "contexts": ["pane"], "id": "hello", "placement": "job", "title": "say hello"}],
                      "api": 1, "description": "A test", "events": [], "id": "gpui.test", "name": "GPUI test",
                      "permissions": ["read", "send"], "platforms": [], "service": [{"id": "s"}], "version": "0.1.0", "window": []},
         "source": {"kind": "link", "path": "/code/test"}},
        {"enabled": false, "error": "bin/x is missing", "id": "acme.notes", "manifest": null,
         "source": {"kind": "git", "repo": "acme/notes", "url": "https://github.com/acme/notes.git", "ref": "v1",
                    "subdir": "plugins/notes", "commit": "0123456789abcdef0123456789abcdef01234567", "path": "/p"}}
    ]}"#;
    let list: List = serde_json::from_str(text).expect("the plugin list parses");
    assert_linked(&list.plugins[0]);
    assert_installed(&list.plugins[1]);
    assert_eq!(Manifest::default().asks(), "asks for nothing");
}

fn assert_linked(a: &Plugin) {
    assert_eq!(a.name(), "GPUI test");
    assert!(!a.source.installed());
    assert_eq!(a.source.describe(), "linked from /code/test");
    let m = a.manifest.as_ref().expect("a manifest");
    assert_eq!(m.asks(), "asks for read, send");
    assert_eq!(m.runs(), ["1 action", "1 service"]);
}

fn assert_installed(b: &Plugin) {
    assert_eq!(b.name(), "acme.notes");
    assert!(b.source.installed());
    assert_eq!(b.source.describe(), "installed from acme/notes/plugins/notes @v1 (0123456789ab)");
    assert_eq!(b.error.as_deref(), Some("bin/x is missing"));
}

fn with_window_process(enabled: bool, error: Option<&str>) -> Plugin {
    let manifest = serde_json::from_str(r#"{"name": "Panel", "window": [{"id": "panel", "command": ["bin/panel"]}]}"#)
        .expect("the manifest parses");
    Plugin { enabled, error: error.map(str::to_string), manifest: Some(manifest), ..Plugin::default() }
}

#[test]
fn a_plugin_that_is_on_keeps_its_window_processes() {
    assert!(with_window_process(true, None).keeps_window_processes());
}

#[test]
fn a_plugin_that_is_off_or_cannot_run_keeps_no_window_process() {
    assert!(!with_window_process(false, None).keeps_window_processes());
    assert!(!with_window_process(true, Some("bin/panel is missing")).keeps_window_processes());
}

#[test]
fn a_plugin_without_window_processes_keeps_none() {
    let plugin: Plugin =
        serde_json::from_str(r#"{"id": "acme.notes", "enabled": true, "manifest": {"name": "Notes", "window": []}}"#)
            .expect("the plugin parses");
    assert!(!plugin.keeps_window_processes());
}

#[test]
fn a_finished_action_says_how_long_it_took() {
    let ok: Ran = serde_json::from_str(r#"{"plugin": "p", "error": null, "ms": 380, "ended": "exited", "code": 0}"#)
        .expect("the outcome parses");
    let (text, kind) = ok.say("say hello").expect("it says so");
    assert_eq!(text, "say hello: done in 380 ms");
    assert!(kind == ToastKind::Info);
    assert_eq!(duration(1500), "1.5 s");
}

#[test]
fn a_failed_action_points_at_its_log() {
    let failed: Ran =
        serde_json::from_str(crate::test_util::fixture!("plugins/run-failed.json")).expect("the failure parses");
    let (text, kind) = failed.say("Fail on purpose").expect("it says so");
    assert_eq!(text, "Fail on purpose: exit status: 3; `cornercase plugin logs acme.notes` says more");
    assert!(kind == ToastKind::Error);
}

#[test]
fn a_popup_closed_by_hand_says_nothing() {
    let closed: Ran =
        serde_json::from_str(crate::test_util::fixture!("plugins/run-closed.json")).expect("the outcome parses");
    assert!(closed.say("Open board").is_none());
}
