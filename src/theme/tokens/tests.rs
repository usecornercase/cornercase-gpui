use super::*;

#[test]
fn every_token_can_be_read_and_set_by_its_key() {
    let mut chrome = Chrome::blank();
    let white = from_bytes([255, 255, 255]);
    for token in TOKENS {
        assert!(chrome.set(token.key, white), "{}", token.key);
        assert_eq!(chrome.get(token.key), Some(white), "{}", token.key);
    }
    assert!(!chrome.set("nothing", white));
}

#[test]
fn opacities_stay_within_their_limits() {
    let mut opacity = Opacity { ground: 0.6, panes: 0.6, cards: 0.3, floating: 0.9, veil: 0.5 };
    assert!(opacity.set("ground", 0.1));
    assert!((opacity.ground - 0.55).abs() < f32::EPSILON);
    assert!(opacity.set("cards", 4.));
    assert!((opacity.cards - 0.8).abs() < f32::EPSILON);
    assert!(!opacity.set("sky", 0.5));
}
