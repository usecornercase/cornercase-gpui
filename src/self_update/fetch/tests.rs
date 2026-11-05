use super::*;

#[test]
fn nothing_published_is_not_an_error() {
    assert_eq!(read_status(404), Ok(false));
    assert_eq!(read_status(200), Ok(true));
}

#[test]
fn githubs_limit_says_when_the_next_check_comes() {
    for status in [403, 429] {
        assert!(read_status(status).expect_err("a limit").contains("the next check is in an hour"));
    }
}

#[test]
fn another_status_is_an_error() {
    assert_eq!(read_status(500), Err("GitHub answered 500".to_string()));
}

#[test]
fn only_an_http_address_asked_on_purpose_allows_plain_downloads() {
    assert!(secure("https://api.github.com/x"));
    assert!(!secure("http://127.0.0.1:8765/latest.json"));
}
