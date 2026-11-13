use serde_json::json;

use super::*;

#[test]
fn the_page_sees_a_plain_chrome_not_a_headless_one() {
    let agent = "Mozilla/5.0 (Macintosh) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/156.0.0.0 Safari/537.36";
    assert_eq!(
        plain_agent(agent),
        "Mozilla/5.0 (Macintosh) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36"
    );
}

#[test]
fn a_bridge_call_carries_the_page_message() {
    let params = json!({ "name": BRIDGE, "payload": "{\"kind\":\"cursor\",\"cursor\":\"pointer\"}" });
    assert_eq!(bridged(&params), Some(json!({ "kind": "cursor", "cursor": "pointer" })));
}

#[test]
fn another_binding_is_not_ours() {
    assert_eq!(bridged(&json!({ "name": "other", "payload": "{}" })), None);
}

#[test]
fn a_restored_tab_reopens_through_its_hold() {
    use crate::model::Owner;
    use crate::panels::browser::tabs::{Restore, Through};
    let through = Some(Through { remote: 8765, local: 61000 });
    let held = Restore { url: "http://localhost:61000/a".into(), owner: Owner::Here(1), through };
    assert_eq!(restore_hold(&held), Some((8765, "http://localhost:8765/a".to_string())));
    let moved = Restore { url: "https://example.com/".into(), ..held.clone() };
    assert_eq!(restore_hold(&moved), None);
    let plain = Restore { through: None, ..held };
    assert_eq!(restore_hold(&plain), None);
}
