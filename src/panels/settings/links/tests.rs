use super::*;
use crate::panels::settings::data;

#[test]
fn the_apps_own_settings_are_choices_on_the_links_page() {
    let settings = settings(&Prefs::default());
    let keys: Vec<&str> = settings.iter().map(|s| s.key.as_str()).collect();
    assert_eq!(keys, ["app.open_links", "app.preview_pages"]);
    assert!(settings.iter().all(|s| s.page == LINKS && s.kind == Kind::Choice && s.as_segments()));
}

#[test]
fn a_setting_shows_its_value_and_its_default() {
    let prefs = Prefs::default().with("open_links", "browser_panel").expect("a known value");
    let open = &settings(&prefs)[0];
    assert_eq!((open.text(), data::default_shown(open)), ("browser_panel".to_string(), "default browser".to_string()));
    assert!(!open.is_default());
}

#[test]
fn only_the_apps_own_keys_are_saved_here() {
    assert_eq!(owned("app.open_links"), Some("open_links"));
    assert_eq!(owned("open_links"), None);
    assert_eq!(owned("app.font_size"), None);
}

#[test]
fn adding_them_again_replaces_the_old_values() {
    let mut described = Described::default();
    add(&mut described, &Prefs::default());
    add(&mut described, &Prefs::default().with("preview_pages", "when_asked").expect("a known value"));
    assert_eq!(described.settings.len(), 2);
    assert_eq!(described.settings[1].text(), "when_asked");
}

#[test]
fn the_links_page_is_found_by_its_settings() {
    let mut described = Described::default();
    add(&mut described, &Prefs::default());
    let found = data::search(&described, "preview");
    assert_eq!(found.iter().map(|f| f.page.as_str()).collect::<Vec<_>>(), [LINKS]);
}
