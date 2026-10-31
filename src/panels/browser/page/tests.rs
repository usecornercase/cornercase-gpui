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
