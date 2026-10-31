use super::data::{ActionList, List, Manifest, Plugin, Ran, duration, target};
use crate::model::Shown;
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

#[test]
fn reads_actions_and_outcomes() {
    let actions: ActionList = serde_json::from_str(
        r#"{"actions": [{"action": "hello", "contexts": ["workspace", "button"], "name": "GPUI test",
            "placement": "popup", "plugin": "gpui.test", "title": "say hello", "width": 80}], "revision": 2}"#,
    )
    .expect("the actions parse");
    assert_eq!(actions.revision, 2);
    assert_eq!(actions.actions[0].placement, "popup");
    let ok: Ran = serde_json::from_str(r#"{"plugin": "p", "error": null, "ms": 380, "ended": "exited", "code": 0}"#)
        .expect("the outcome parses");
    let (text, kind) = ok.say("say hello");
    assert_eq!(text, "say hello: done in 380 ms");
    assert!(kind == ToastKind::Info);
    let failed: Ran =
        serde_json::from_str(r#"{"plugin": "p", "error": "exit status: 3", "ms": 1500}"#).expect("the failure parses");
    assert_eq!(failed.say("x").0, "x: exit status: 3; `cornercase plugin logs p` says more");
    assert_eq!(duration(1500), "1.5 s");
}

#[test]
fn runs_on_the_most_precise_place_shown() {
    let shown = Shown { project: Some(1), workspace: Some(2), tab: Some(3), pane: Some(4) };
    let ctx = |c: &[&str]| c.iter().map(ToString::to_string).collect::<Vec<_>>();
    assert_eq!(target(&ctx(&["workspace", "pane"]), shown), Some(vec!["--pane".into(), "4".into()]));
    assert_eq!(target(&ctx(&["project", "tab"]), shown), Some(vec!["--tab".into(), "3".into()]));
    assert_eq!(target(&ctx(&["pane", "button"]), shown), Some(vec![]));
    assert_eq!(target(&ctx(&["pane"]), Shown::default()), None);
    assert_eq!(target(&ctx(&[]), shown), None);
}
