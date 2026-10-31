use serde_json::json;

use super::*;

#[test]
fn copying_asks_the_page_for_its_selection() {
    let script = selection_script(false);
    assert_eq!(script["returnByValue"], true);
    assert!(script["expression"].as_str().is_some_and(|e| e.contains("__cornercase.selected()")));
}

#[test]
fn cutting_asks_the_page_to_delete_what_it_returns() {
    assert!(selection_script(true)["expression"].as_str().is_some_and(|e| e.contains("__cornercase.cut()")));
}

#[test]
fn the_selection_comes_back_as_the_value() {
    let reply = json!({ "result": { "type": "string", "value": "hello" } });
    assert_eq!(evaluated_text(&reply).as_deref(), Some("hello"));
}

#[test]
fn an_empty_selection_leaves_the_clipboard_alone() {
    assert_eq!(evaluated_text(&json!({ "result": { "type": "string", "value": "" } })), None);
    assert_eq!(evaluated_text(&json!({ "result": { "type": "undefined" } })), None);
}
