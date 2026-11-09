use std::collections::BTreeMap;

use serde_json::json;

use super::*;
use crate::theme::current::Presets;

struct Both(BTreeMap<String, String>);

impl Lookup for Both {
    fn text(&self, id: &str) -> Option<String> {
        Presets.text(id).or_else(|| self.0.get(id).cloned())
    }
}

#[test]
fn setting_a_token_adds_it_to_its_section() {
    let mut file = ThemeFile::default();
    set(&mut file, Part::Chrome, "brand", json!("#ff0000"));
    assert_eq!(file.chrome.get("brand"), Some(&json!("#ff0000")));
}

#[test]
fn resetting_a_token_removes_it() {
    let mut file = ThemeFile::default();
    set(&mut file, Part::Terminal, "red", json!("#ff0000"));
    remove(&mut file, Part::Terminal, "red");
    assert!(file.terminal.is_empty());
}

#[test]
fn resetting_the_theme_keeps_its_name_and_base() {
    let mut file = copy_of("nord", "Nord");
    set(&mut file, Part::Chrome, "brand", json!("#ff0000"));
    set(&mut file, Part::Opacity, "cards", json!(0.5));
    reset_all(&mut file);
    assert_eq!((file.name.as_deref(), file.extends.as_deref()), (Some("My Nord"), Some("nord")));
    assert!(file.chrome.is_empty() && file.opacity.is_empty());
}

#[test]
fn an_edit_keeps_keys_this_version_does_not_know() {
    let mut file = crate::theme::file::parse(r#"{"later": 1}"#).expect("theme");
    set(&mut file, Part::Chrome, "brand", json!("#ff0000"));
    assert_eq!(file.other.get("later"), Some(&json!(1)));
}

#[test]
fn export_merges_user_bases_and_keeps_a_preset_base() {
    let files = BTreeMap::from([
        ("a".to_string(), r##"{"extends": "b", "chrome": {"text": "#111111"}}"##.to_string()),
        ("b".to_string(), r##"{"extends": "nord", "chrome": {"soft": "#222222"}}"##.to_string()),
    ]);
    let flat = flatten("a", &Both(files)).expect("flat");
    assert_eq!(flat.extends.as_deref(), Some("nord"));
    assert!(flat.chrome.contains_key("text") && flat.chrome.contains_key("soft"));
    assert!(flat.chrome.len() == 2);
}

#[test]
fn a_copy_of_a_preset_extends_it_and_is_named_after_it() {
    let copy = copy_of("gruvbox-dark", "Gruvbox Dark");
    assert_eq!(copy.extends.as_deref(), Some("gruvbox-dark"));
    assert_eq!(copy.name.as_deref(), Some("My Gruvbox Dark"));
}
