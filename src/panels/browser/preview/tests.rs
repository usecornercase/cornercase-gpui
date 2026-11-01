use serde_json::json;

use super::*;

#[test]
fn a_page_of_a_preview_context_is_the_previews_own() {
    let previews = Previews { contexts: vec!["C1".into()] };
    assert!(previews.owns(&json!({ "targetId": "T", "browserContextId": "C1" })));
    assert!(!previews.owns(&json!({ "targetId": "T", "browserContextId": "DEFAULT" })));
    assert!(!previews.owns(&json!({ "targetId": "T" })));
}

#[test]
fn chromes_errors_read_as_what_went_wrong() {
    assert_eq!(failure("net::ERR_CONNECTION_REFUSED"), "Nothing answers at this address.");
    assert_eq!(failure("net::ERR_CERT_AUTHORITY_INVALID"), "Its certificate is not valid.");
    assert_eq!(failure("net::ERR_TIMED_OUT"), "The page did not load: timed out.");
}

#[test]
fn a_page_that_never_answers_says_so_and_other_errors_stay() {
    let late = unanswered(Late("Page.navigate".into()).into());
    assert_eq!(late.to_string(), "The page did not answer within 8 seconds.");
    assert_eq!(unanswered(anyhow!("the browser went away")).to_string(), "the browser went away");
}
