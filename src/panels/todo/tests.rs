use serde_json::json;

use super::data::{Item, drop_on, items, sent_message, short, step, targets, undo_number};
use crate::model::{PaneInfo, Project, Session, Tab, Workspace};
use crate::store::ToastKind;

fn item(id: u64, done: bool) -> Item {
    Item { id, text: format!("item {id}"), done }
}

fn moved(command: Option<Vec<String>>) -> Option<String> {
    command.map(|c| c[2..].join(" "))
}

#[test]
fn reads_the_app_feed() {
    let app = json!({ "todos": [
        { "id": 3, "text": "review the login fix", "done": false },
        { "id": 1, "text": "ship", "done": true },
        { "text": "no id" },
    ], "queue": [] });
    assert_eq!(
        items(Some(&app)),
        vec![
            Item { id: 3, text: "review the login fix".into(), done: false },
            Item { id: 1, text: "ship".into(), done: true }
        ]
    );
    assert_eq!(items(Some(&json!({}))), vec![]);
    assert_eq!(items(None), vec![]);
}

#[test]
fn reads_the_undo_number() {
    assert_eq!(undo_number(&json!({ "todos": [{ "id": 1, "text": "x", "done": true }], "undo": 7 })), Some(7));
    assert_eq!(undo_number(&json!({ "todos": [] })), None);
}

#[test]
fn steps_within_its_kind() {
    let list = [item(1, false), item(2, false), item(3, false), item(4, true), item(5, true)];
    assert_eq!(moved(step(&list, 1, true)), None);
    assert_eq!(moved(step(&list, 2, true)).as_deref(), Some("2 --before 1"));
    assert_eq!(moved(step(&list, 1, false)).as_deref(), Some("1 --before 3"));
    assert_eq!(moved(step(&list, 2, false)).as_deref(), Some("2 --last"));
    assert_eq!(moved(step(&list, 3, false)), None);
    assert_eq!(moved(step(&list, 4, true)), None);
    assert_eq!(moved(step(&list, 4, false)).as_deref(), Some("4 --last"));
    assert_eq!(moved(step(&list, 5, true)).as_deref(), Some("5 --before 4"));
    assert_eq!(moved(step(&list, 9, true)), None);
}

#[test]
fn drops_before_above_and_after_below() {
    let list = [item(1, false), item(2, false), item(3, false), item(4, true)];
    assert_eq!(moved(drop_on(&list, 3, 1)).as_deref(), Some("3 --before 1"));
    assert_eq!(moved(drop_on(&list, 1, 2)).as_deref(), Some("1 --before 3"));
    assert_eq!(moved(drop_on(&list, 1, 4)).as_deref(), Some("1 --last"));
    assert_eq!(moved(drop_on(&list, 2, 2)), None);
}

fn pane(id: u64, agent: Option<&str>) -> PaneInfo {
    PaneInfo { id, agent: agent.map(Into::into), status: Some("idle".into()), ..Default::default() }
}

fn tab(name: &str, panes: Vec<PaneInfo>) -> Tab {
    Tab { name: name.into(), panes, ..Default::default() }
}

fn workspace(name: &str, tabs: Vec<Tab>) -> Workspace {
    Workspace { name: name.into(), tabs, ..Default::default() }
}

fn project(name: &str, workspaces: Vec<Workspace>) -> Project {
    Project { name: name.into(), workspaces, ..Default::default() }
}

#[test]
fn labels_agents_like_the_tui() {
    let shop = project(
        "web-shop",
        vec![
            workspace("default", vec![tab("claude", vec![pane(1, Some("claude")), pane(2, None)])]),
            workspace(
                "feat/x",
                vec![
                    tab("a", vec![pane(3, Some("codex"))]),
                    tab("b", vec![pane(4, Some("codex"))]),
                    tab("b", vec![pane(5, Some("codex"))]),
                ],
            ),
        ],
    );
    let api = project("api", vec![workspace("", vec![tab("", vec![pane(6, Some("claude"))])])]);
    let session = Session { projects: vec![shop, api], ..Default::default() };
    let labels: Vec<(u64, String)> = targets(&session).into_iter().map(|t| (t.pane, t.label)).collect();
    assert_eq!(
        labels,
        vec![
            (1, "web-shop › default · claude".to_string()),
            (3, "web-shop › feat/x · codex (a)".to_string()),
            (4, "web-shop › feat/x · codex (b) (pane 4)".to_string()),
            (5, "web-shop › feat/x · codex (b) (pane 5)".to_string()),
            (6, "api · claude".to_string()),
        ]
    );
}

#[test]
fn says_how_a_send_went() {
    assert!(matches!(sent_message(&json!({ "pane": 5 }), "x").1, ToastKind::Done));
    assert_eq!(sent_message(&json!({ "unconfirmed": true }), "x").0, "sent to x, not confirmed: stays to do");
    assert!(matches!(sent_message(&json!({ "checked": false }), "x").1, ToastKind::Info));
}

#[test]
fn shortens_long_texts() {
    assert_eq!(short("short", 10), "short");
    assert_eq!(short("a rather long item text", 10), "a rather…");
}
