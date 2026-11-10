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

#[test]
fn the_palette_is_not_pushed_to_a_server_the_window_is_not_connected_to() {
    let older =
        crate::cli::Older { destination: "vps".into(), there: None, here: "0.13.28".into(), update: Vec::new() };
    assert!(takes_palette(&Conn::Connected));
    assert!(!takes_palette(&Conn::Starting));
    assert!(!takes_palette(&Conn::Older(older)));
    assert!(!takes_palette(&Conn::Lost("x".into())));
}
