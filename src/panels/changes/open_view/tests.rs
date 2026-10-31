use super::*;

#[test]
fn the_diff_shown_names_the_version_acted_on() {
    assert_eq!(shown_digest(Some("6321f7f34ac2b1b9".into()), "d5dccd1bdadf10a2"), "6321f7f34ac2b1b9");
}

#[test]
fn without_a_diff_read_the_listed_version_is_acted_on() {
    assert_eq!(shown_digest(None, "d5dccd1bdadf10a2"), "d5dccd1bdadf10a2");
    assert_eq!(shown_digest(Some(String::new()), "d5dccd1bdadf10a2"), "d5dccd1bdadf10a2");
}
