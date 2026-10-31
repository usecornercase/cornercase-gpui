use serde_json::json;

use super::*;

fn opening(kind: &str, url: &str) -> Dialog {
    let params =
        json!({ "url": url, "message": "Proceed?", "type": kind, "hasBrowserHandler": true, "defaultPrompt": "Ada" });
    dialog(&params, 1).expect("a dialog")
}

#[test]
fn an_alert_says_where_it_comes_from_and_only_offers_ok() {
    let alert = opening("alert", "http://localhost:8783/dialogs.html");
    assert_eq!(alert.title(), "localhost:8783 says");
    assert_eq!(alert.body(), "Proceed?");
    assert_eq!(alert.buttons(), (None, "OK"));
}

#[test]
fn a_prompt_keeps_its_default_answer() {
    let prompt = opening("prompt", "https://example.com/a?b");
    assert_eq!((prompt.kind, prompt.default.as_str()), (Kind::Prompt, "Ada"));
    assert_eq!(prompt.buttons(), (Some("Cancel"), "OK"));
}

#[test]
fn leaving_a_page_asks_in_the_browser_s_own_words() {
    let leave = opening("beforeunload", "http://localhost:5173/");
    assert_eq!(leave.title(), "Leave this page?");
    assert_eq!(leave.body(), LEAVE);
    assert_eq!(leave.buttons(), (Some("Stay"), "Leave"));
}

#[test]
fn a_page_without_a_host_is_this_page() {
    assert_eq!(opening("confirm", "file:///tmp/a.html").title(), "This page says");
}

#[test]
fn credentials_in_the_address_are_not_shown() {
    assert_eq!(origin("http://user:secret@localhost:8000/x"), "localhost:8000");
}

#[test]
fn an_unknown_kind_of_dialog_is_ignored() {
    assert_eq!(dialog(&json!({ "type": "print", "url": "http://a/" }), 1), None);
}
