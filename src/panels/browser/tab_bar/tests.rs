use super::*;

#[test]
fn a_blank_tab_is_a_new_tab() {
    assert_eq!(tidy("about:blank"), "New tab");
    assert_eq!(tidy("  "), "New tab");
}

#[test]
fn a_title_is_kept_as_written() {
    assert_eq!(tidy("Dashboard / Acme"), "Dashboard / Acme");
}

#[test]
fn an_address_loses_its_scheme_and_last_slash() {
    assert_eq!(tidy("http://localhost:5173/"), "localhost:5173");
    assert_eq!(tidy("https://example.com/docs/"), "example.com/docs");
    assert_eq!(tidy("localhost:8782/"), "localhost:8782");
}

#[test]
fn the_tooltip_gives_the_title_and_the_address() {
    assert_eq!(detail("Docs", "https://example.com/docs"), "Docs\nhttps://example.com/docs");
}

#[test]
fn the_tooltip_of_an_untitled_page_is_its_address() {
    assert_eq!(detail("", "http://localhost:5173/"), "localhost:5173");
    assert_eq!(detail("http://a.test/", "http://a.test/"), "a.test");
    assert_eq!(detail("", "about:blank"), "New tab");
}

#[test]
fn the_active_tab_alone_is_lit() {
    let tab = Tab::new("A", "http://a.test/", None);
    assert!(Face::of(&tab, Some("A"), false).active);
    assert!(!Face::of(&tab, Some("B"), false).active);
}
