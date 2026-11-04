use super::*;

#[test]
fn a_machine_without_chrome_is_told_how_to_fix_it() {
    assert!(chrome_missing(chrome::NO_CHROME));
}

#[test]
fn another_failure_shows_its_own_reason() {
    assert!(!chrome_missing("the browser did not open its DevTools port within 20 s"));
}

#[test]
fn on_windows_the_help_sets_the_variable_for_the_next_start_and_names_an_exe() {
    assert_eq!(variable_hint(Os::Windows), r#"setx CORNERCASE_GPUI_CHROME "C:\path\to\chrome.exe""#);
    assert_eq!(variable_hint(Os::Linux), "CORNERCASE_GPUI_CHROME=/path/to/chrome");
}
