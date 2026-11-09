use super::*;

#[test]
fn a_palette_already_given_is_not_given_again() {
    let given = Given::of("a".to_string());
    assert!(!given.needs("a"));
    assert!(given.needs("b"));
}

#[test]
fn a_restarted_server_is_given_the_palette_again() {
    let mut given = Given::of("a".to_string());
    given.forget();
    assert!(given.needs("a"));
    given.gave("a".to_string());
    assert!(!given.needs("a"));
}
