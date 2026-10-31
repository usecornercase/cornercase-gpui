use serde_json::{Value, json};

use super::*;

#[test]
fn a_request_names_its_session_when_it_has_one() {
    let text = request(7, Some("S1"), "Page.navigate", json!({ "url": "http://localhost:3000" }));
    let value: Value = serde_json::from_str(&text).expect("json");
    assert_eq!(
        value,
        json!({ "id": 7, "method": "Page.navigate", "params": { "url": "http://localhost:3000" }, "sessionId": "S1" })
    );
}

#[test]
fn a_request_without_params_sends_an_empty_object() {
    let value: Value = serde_json::from_str(&request(1, None, "Target.getTargets", Value::Null)).expect("json");
    assert_eq!(value, json!({ "id": 1, "method": "Target.getTargets", "params": {} }));
}

#[test]
fn a_reply_carries_its_result() {
    let parsed = parse(r#"{"id":3,"result":{"frameId":"F"}}"#);
    assert_eq!(parsed, Some(Parsed::Reply(3, Ok(json!({ "frameId": "F" })))));
}

#[test]
fn an_error_reply_carries_its_message() {
    let parsed = parse(r#"{"id":4,"error":{"code":-32000,"message":"No target"}}"#);
    assert_eq!(parsed, Some(Parsed::Reply(4, Err("No target".into()))));
}

#[test]
fn an_event_keeps_its_session_and_params() {
    let parsed = parse(r#"{"method":"Page.loadEventFired","params":{"timestamp":1.5},"sessionId":"S2"}"#);
    let event =
        Event { session: Some("S2".into()), method: "Page.loadEventFired".into(), params: json!({ "timestamp": 1.5 }) };
    assert_eq!(parsed, Some(Parsed::Event(event)));
}

#[test]
fn a_line_that_is_not_json_is_ignored() {
    assert_eq!(parse("not json"), None);
}
