use serde_json::{Value, json};

use super::*;
use crate::test_util::fixture;

const MAIN: &str = "8B2DC1AEEFDBA28B02E3D316EDD9ECD6";

fn read(text: &str) -> Value {
    serde_json::from_str(text).expect("a fixture is JSON")
}

fn logged(log: &Log) -> Vec<(Level, String, u32)> {
    log.entries().map(|l| (l.entry.level, l.entry.text.clone(), l.times)).collect()
}

fn said(text: &str) -> Value {
    json!({ "type": "log", "args": [{ "type": "string", "value": text }] })
}

#[test]
fn a_console_call_reads_as_one_line_with_its_format_filled_in() {
    let entry = console_entry(&read(fixture!("browser/console-format.json"))).expect("an entry");
    assert_eq!(entry.level, Level::Info);
    assert_eq!(entry.text, "hello world, you are 42 {a: 1, b: \"two\", c: Array(3)}");
    assert_eq!(entry.source.as_deref(), Some("http://localhost:8783/errors.html:5:9"));
    assert_eq!(entry.stack, Vec::<String>::new());
}

#[test]
fn a_console_error_keeps_the_first_line_of_an_error_and_its_stack() {
    let entry = console_entry(&read(fixture!("browser/console-error.json"))).expect("an entry");
    assert_eq!(entry.level, Level::Error);
    assert_eq!(entry.text, "an error Error: inner boom");
    assert_eq!(entry.stack, ["at http://localhost:8783/errors.html:8:9"]);
}

#[test]
fn a_console_warning_is_a_warning() {
    let entry = console_entry(&read(fixture!("browser/console-warning.json"))).expect("an entry");
    assert_eq!(entry.level, Level::Warning);
    assert_eq!(entry.text, "a warning Map(1) {1 => 2}");
}

#[test]
fn a_failed_assertion_says_so() {
    let entry = console_entry(&json!({ "type": "assert", "args": [{ "type": "string", "value": "x > 0" }] }));
    assert_eq!(entry.map(|e| (e.level, e.text)), Some((Level::Error, "Assertion failed: x > 0".to_string())));
}

#[test]
fn the_end_of_a_group_logs_nothing() {
    assert_eq!(console_entry(&json!({ "type": "endGroup", "args": [] })), None);
}

#[test]
fn an_uncaught_error_names_its_type_message_and_where_it_was_thrown() {
    let entry = exception_entry(&read(fixture!("browser/exception.json"))["exceptionDetails"]);
    assert_eq!(entry.text, "Uncaught TypeError: Cannot read properties of undefined (reading 'x')");
    assert_eq!(entry.source.as_deref(), Some("http://localhost:8783/errors.html:10:37"));
    assert_eq!(
        entry.stack,
        ["at crash (http://localhost:8783/errors.html:10:37)", "at later (http://localhost:8783/errors.html:11:31)"]
    );
}

#[test]
fn a_thrown_string_reads_as_the_string() {
    let entry = exception_entry(&read(fixture!("browser/exception-string.json"))["exceptionDetails"]);
    assert_eq!(entry.text, "Uncaught a string");
}

#[test]
fn an_exception_whose_text_holds_the_message_does_not_repeat_it() {
    let details = json!({
        "text": "Uncaught SyntaxError: Unexpected token '}'",
        "url": "http://localhost/app.js", "lineNumber": 3, "columnNumber": 0,
        "exception": { "type": "object", "subtype": "error", "description": "SyntaxError: Unexpected token '}'" }
    });
    let entry = exception_entry(&details);
    assert_eq!(entry.text, "Uncaught SyntaxError: Unexpected token '}'");
    assert_eq!(entry.source.as_deref(), Some("http://localhost/app.js:4:1"));
}

#[test]
fn a_browser_message_keeps_its_level_and_url() {
    let entry =
        log_entry(&json!({ "source": "security", "level": "warning", "text": "Mixed content", "url": "https://a" }));
    assert_eq!(
        entry.map(|e| (e.level, e.text, e.source)),
        Some((Level::Warning, "Mixed content".into(), Some("https://a".into())))
    );
}

#[test]
fn a_verbose_browser_message_is_left_out() {
    assert_eq!(log_entry(&json!({ "source": "violation", "level": "verbose", "text": "slow handler" })), None);
}

#[test]
fn a_failed_response_names_its_method_url_and_status_once() {
    let mut log = Log::default();
    log.apply("Network.requestWillBeSent", &read(fixture!("browser/request-fetch.json")), MAIN);
    log.apply("Network.responseReceived", &read(fixture!("browser/response-404.json")), MAIN);
    log.apply("Log.entryAdded", &read(fixture!("browser/log-network.json")), MAIN);
    assert_eq!(logged(&log), [(Level::Error, "GET http://localhost:8783/missing.json 404 File not found".into(), 1)]);
}

#[test]
fn a_browser_message_about_a_request_seen_before_attaching_is_kept() {
    let mut log = Log::default();
    log.apply("Log.entryAdded", &read(fixture!("browser/log-network.json")), MAIN);
    assert_eq!(log.errors(), 1);
}

#[test]
fn a_load_that_failed_names_its_url_and_error() {
    let mut log = Log::default();
    let request = json!({ "requestId": "7", "loaderId": "L", "type": "Fetch", "frameId": MAIN,
        "request": { "method": "POST", "url": "http://localhost:1/api" } });
    log.apply("Network.requestWillBeSent", &request, MAIN);
    log.apply("Network.loadingFailed", &json!({ "requestId": "7", "errorText": "net::ERR_CONNECTION_REFUSED" }), MAIN);
    assert_eq!(logged(&log), [(Level::Error, "POST http://localhost:1/api net::ERR_CONNECTION_REFUSED".into(), 1)]);
}

#[test]
fn a_cancelled_load_is_not_an_error() {
    let mut log = Log::default();
    let request = json!({ "requestId": "7", "request": { "method": "GET", "url": "http://a/b" } });
    log.apply("Network.requestWillBeSent", &request, MAIN);
    log.apply(
        "Network.loadingFailed",
        &json!({ "requestId": "7", "errorText": "net::ERR_ABORTED", "canceled": true }),
        MAIN,
    );
    assert_eq!(log.errors(), 0);
}

#[test]
fn a_missing_favicon_is_not_an_error() {
    let mut log = Log::default();
    let response =
        json!({ "requestId": "9", "response": { "url": "http://localhost:8783/favicon.ico", "status": 404 } });
    log.apply("Network.responseReceived", &response, MAIN);
    assert_eq!(log.errors(), 0);
}

#[test]
fn a_new_document_in_the_main_frame_clears_the_log() {
    let mut log = Log::default();
    log.apply("Runtime.consoleAPICalled", &said("old page"), MAIN);
    log.apply("Network.requestWillBeSent", &read(fixture!("browser/request-document.json")), MAIN);
    assert_eq!(log.len(), 0);
}

#[test]
fn a_document_loading_in_a_frame_keeps_the_log() {
    let mut log = Log::default();
    log.apply("Runtime.consoleAPICalled", &said("page"), MAIN);
    let frame = json!({ "requestId": "F", "loaderId": "F", "type": "Document", "frameId": "IFRAME",
        "request": { "method": "GET", "url": "http://a/frame" } });
    log.apply("Network.requestWillBeSent", &frame, MAIN);
    assert_eq!(log.len(), 1);
}

#[test]
fn the_error_page_of_a_failed_navigation_keeps_its_failure() {
    let mut log = Log::default();
    let document = read(fixture!("browser/request-document.json"));
    log.apply("Network.requestWillBeSent", &document, MAIN);
    log.apply(
        "Network.loadingFailed",
        &json!({ "requestId": document["requestId"], "errorText": "net::ERR_CONNECTION_REFUSED" }),
        MAIN,
    );
    let frame =
        json!({ "frame": { "id": MAIN, "loaderId": document["loaderId"], "url": "chrome-error://chromewebdata/" } });
    log.apply("Page.frameNavigated", &frame, MAIN);
    assert_eq!(log.errors(), 1);
}

#[test]
fn a_document_restored_without_a_request_clears_the_log() {
    let mut log = Log::default();
    log.apply("Runtime.consoleAPICalled", &said("page"), MAIN);
    log.apply("Page.frameNavigated", &json!({ "frame": { "id": MAIN, "loaderId": "BFCACHE" } }), MAIN);
    assert_eq!(log.len(), 0);
}

#[test]
fn console_clear_empties_the_log() {
    let mut log = Log::default();
    log.apply("Runtime.consoleAPICalled", &said("before"), MAIN);
    log.apply("Runtime.consoleAPICalled", &json!({ "type": "clear", "args": [] }), MAIN);
    assert_eq!(log.len(), 0);
}

#[test]
fn the_same_message_in_a_row_is_counted_not_repeated() {
    let mut log = Log::default();
    for _ in 0..3 {
        log.apply("Runtime.consoleAPICalled", &said("repeat"), MAIN);
    }
    assert_eq!(logged(&log), [(Level::Info, "repeat".into(), 3)]);
}

#[test]
fn the_log_keeps_only_its_latest_entries_and_counts_what_it_keeps() {
    let mut log = Log::default();
    log.add(Some(Entry::new(Level::Error, "first")));
    for n in 0..KEPT {
        log.add(Some(Entry::new(Level::Info, &format!("line {n}"))));
    }
    assert_eq!(log.entries().count(), KEPT);
    assert_eq!(log.errors(), 0);
}

#[test]
fn errors_and_warnings_are_counted_apart() {
    let mut log = Log::default();
    log.add(Some(Entry::new(Level::Error, "a")));
    log.add(Some(Entry::new(Level::Error, "b")));
    log.add(Some(Entry::new(Level::Warning, "c")));
    assert_eq!((log.errors(), log.warnings()), (2, 1));
}

#[test]
fn a_data_url_is_shortened_to_its_type() {
    assert_eq!(short_url("data:image/png;base64,iVBORw0KGgo"), "data:image/png;base64,…");
}

#[test]
fn a_long_text_is_cut_on_a_character() {
    assert_eq!(cut("éééé", 2), "éé…");
}

#[test]
fn the_filters_show_one_level_or_all() {
    assert!(Filter::Errors.shows(Level::Error) && !Filter::Errors.shows(Level::Warning));
    assert!(Filter::Warnings.shows(Level::Warning) && !Filter::Warnings.shows(Level::Info));
    assert!(Filter::All.shows(Level::Info));
}

fn entry_with(id: u64, level: Level, text: &str, stack: &[&str]) -> Logged {
    let mut entry = Entry::new(level, text);
    entry.stack = stack.iter().map(|s| (*s).to_string()).collect();
    Logged { id, entry: Rc::new(entry), times: 1 }
}

#[test]
fn errors_handed_to_an_agent_carry_the_page_and_their_stacks() {
    let a = entry_with(1, Level::Error, "Uncaught TypeError: x", &["at f (http://a/app.js:1:2)"]);
    let b = entry_with(2, Level::Error, "GET http://a/api 500", &[]);
    assert_eq!(
        hand_off("http://a/", &[&a, &b]),
        "The browser's console at http://a/ shows 2 errors:\n1. Uncaught TypeError: x\n   at f (http://a/app.js:1:2)\n2. GET http://a/api 500"
    );
}

#[test]
fn one_entry_handed_to_an_agent_says_what_it_is() {
    let mut warning = entry_with(1, Level::Warning, "deprecated", &[]);
    warning.times = 4;
    Rc::make_mut(&mut warning.entry).source = Some("http://a/x.js:3".into());
    assert_eq!(
        hand_off("http://a/", &[&warning]),
        "The browser's console at http://a/ shows this warning:\n1. deprecated (×4)\n   at http://a/x.js:3"
    );
}

#[test]
fn what_is_handed_to_an_agent_is_bounded() {
    let frames: Vec<String> = (0..10).map(|n| format!("at f{n} (http://a/app.js:{n}:1)")).collect();
    let frames: Vec<&str> = frames.iter().map(String::as_str).collect();
    let many: Vec<Logged> = (0..30).map(|n| entry_with(n, Level::Error, &"x".repeat(1000), &frames)).collect();
    let refs: Vec<&Logged> = many.iter().collect();
    let text = hand_off("http://a/", &refs);
    assert!(text.ends_with("(and 10 earlier, left out)"));
    assert_eq!(text.matches("\n   …").count(), HANDED);
    assert!(text.len() < 20_000, "{}", text.len());
}
