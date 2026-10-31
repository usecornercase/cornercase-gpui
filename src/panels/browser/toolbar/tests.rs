use super::*;

#[test]
fn web_and_file_pages_open_in_the_system_browser() {
    assert_eq!(outside_url("http://localhost:5173/"), Some("http://localhost:5173/"));
    assert_eq!(outside_url("https://example.com"), Some("https://example.com"));
    assert_eq!(outside_url("file:///tmp/a.html"), Some("file:///tmp/a.html"));
}

#[test]
fn blank_and_internal_pages_stay_here() {
    assert_eq!(outside_url("about:blank"), None);
    assert_eq!(outside_url("chrome://version"), None);
    assert_eq!(outside_url(""), None);
}
