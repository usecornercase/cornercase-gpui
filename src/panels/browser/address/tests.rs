use super::*;

#[test]
fn a_full_url_stays_as_typed() {
    assert_eq!(normalize("https://example.com/a?b=1").as_deref(), Some("https://example.com/a?b=1"));
}

#[test]
fn a_bare_port_opens_on_localhost() {
    assert_eq!(normalize("5173").as_deref(), Some("http://localhost:5173"));
    assert_eq!(normalize(":3000").as_deref(), Some("http://localhost:3000"));
}

#[test]
fn local_hosts_open_over_http() {
    assert_eq!(normalize("localhost:8080/admin").as_deref(), Some("http://localhost:8080/admin"));
    assert_eq!(normalize("127.0.0.1:4000").as_deref(), Some("http://127.0.0.1:4000"));
    assert_eq!(normalize("shop.test").as_deref(), Some("http://shop.test"));
}

#[test]
fn other_hosts_open_over_https() {
    assert_eq!(normalize("usecornercase.dev").as_deref(), Some("https://usecornercase.dev"));
}

#[test]
fn words_become_a_search() {
    assert_eq!(normalize("tailwind grid").as_deref(), Some("https://www.google.com/search?q=tailwind+grid"));
    assert_eq!(normalize("rust&gpui").as_deref(), Some("https://www.google.com/search?q=rust%26gpui"));
}

#[test]
fn about_pages_keep_their_scheme() {
    assert_eq!(normalize("about:blank").as_deref(), Some("about:blank"));
}

#[test]
fn an_empty_address_goes_nowhere() {
    assert_eq!(normalize("   "), None);
}
