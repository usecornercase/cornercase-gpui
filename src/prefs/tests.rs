use super::*;

#[test]
fn without_a_file_links_open_in_the_default_browser_and_local_pages_preview() {
    let prefs = Prefs::parse("");
    assert_eq!((prefs.open_links, prefs.preview_pages), (OpenLinks::DefaultBrowser, PreviewPages::LocalAddresses));
}

#[test]
fn the_file_keeps_the_values_by_name() {
    let prefs = Prefs::parse(r#"{"open_links": "browser_panel", "preview_pages": "when_asked"}"#);
    assert_eq!((prefs.open_links, prefs.preview_pages), (OpenLinks::BrowserPanel, PreviewPages::WhenAsked));
    assert_eq!(Prefs::parse(&serde_json::to_string(&prefs).expect("writes")), prefs);
}

#[test]
fn a_value_this_version_does_not_know_falls_back_to_the_default() {
    let prefs = Prefs::parse(r#"{"open_links": "a_tab_of_its_own", "preview_pages": "every_address"}"#);
    assert_eq!((prefs.open_links, prefs.preview_pages), (OpenLinks::DefaultBrowser, PreviewPages::EveryAddress));
}

#[test]
fn a_setting_changes_by_its_key_and_value() {
    let prefs = Prefs::default().with("open_links", "browser_panel").expect("a known value");
    assert_eq!(prefs.open_links, OpenLinks::BrowserPanel);
    assert_eq!(prefs.values()["open_links"], "browser_panel");
}

#[test]
fn an_unknown_key_or_value_is_refused() {
    assert!(Prefs::default().with("font_size", "14").is_err());
    assert!(Prefs::default().with("open_links", "a_tab_of_its_own").is_err());
}

#[test]
fn pages_load_on_hover_as_the_setting_says() {
    let with = |value: &str| Prefs::default().with("preview_pages", value).expect("a known value");
    assert_eq!((Prefs::default().previews(true), Prefs::default().previews(false)), (true, false));
    assert!(with("every_address").previews(false));
    assert!(!with("when_asked").previews(true));
}

#[cfg(target_os = "macos")]
#[test]
fn on_a_mac_the_app_keeps_its_files_in_application_support() {
    use std::path::{Path, PathBuf};

    assert_eq!(data_dir(Path::new("/Users/me")), PathBuf::from("/Users/me/Library/Application Support"));
}

#[test]
fn a_chosen_cornercase_is_kept_in_the_settings() {
    let prefs = Prefs::default().with("cornercase_bin", "/opt/cc/cornercase").expect("a string setting");
    assert_eq!(prefs.cornercase_bin.as_deref(), Some("/opt/cc/cornercase"));
    assert_eq!(Prefs::parse(&serde_json::to_string(&prefs).expect("writes")), prefs);
}

#[test]
fn settings_from_before_keep_no_chosen_cornercase() {
    let prefs = Prefs::parse(r#"{"open_links": "browser_panel"}"#);
    assert_eq!((prefs.open_links, prefs.cornercase_bin), (OpenLinks::BrowserPanel, None));
}
