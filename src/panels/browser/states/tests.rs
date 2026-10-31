use super::*;

#[test]
fn a_machine_without_chrome_is_told_how_to_fix_it() {
    assert!(chrome_missing(chrome::NO_CHROME));
}

#[test]
fn another_failure_shows_its_own_reason() {
    assert!(!chrome_missing("the browser did not open its DevTools port within 20 s"));
}
