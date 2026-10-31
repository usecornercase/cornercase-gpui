use super::file_of;

#[test]
fn a_file_link_names_its_path_whatever_its_host() {
    assert_eq!(file_of("file:///home/me/a.rs").as_deref(), Some("/home/me/a.rs"));
    assert_eq!(file_of("file://devbox/home/me/a.rs").as_deref(), Some("/home/me/a.rs"));
    assert_eq!(file_of("file:/home/me/a.rs").as_deref(), Some("/home/me/a.rs"));
}

#[test]
fn a_file_link_reads_its_escapes() {
    assert_eq!(file_of("file:///home/me/my%20notes%2Fold.md").as_deref(), Some("/home/me/my notes/old.md"));
    assert_eq!(file_of("file:///home/me/caf%C3%A9.txt").as_deref(), Some("/home/me/café.txt"));
    assert_eq!(file_of("file:///home/me/100%.txt").as_deref(), Some("/home/me/100%.txt"));
    assert_eq!(file_of("file:///home/me/a%+1").as_deref(), Some("/home/me/a%+1"));
}

#[test]
fn only_a_file_link_names_a_path() {
    assert_eq!(file_of("https://example.com/a"), None);
    assert_eq!(file_of("file://devbox"), None);
    assert_eq!(file_of("file:relative"), None);
}
