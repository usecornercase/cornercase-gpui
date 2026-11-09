use super::*;
use crate::theme::current::Presets;
use crate::theme::resolve::resolve;

#[test]
fn the_built_in_fallback_is_cornercase_dark_and_resolves() {
    assert_eq!(crate::theme::current::fallback().id, crate::theme::resolve::ROOT);
    assert!(is_preset(crate::theme::resolve::ROOT));
}

#[test]
fn every_preset_resolves_without_warnings() {
    for (id, _) in PRESETS {
        let resolved = resolve(id, &Presets).expect(id);
        assert!(resolved.warnings.is_empty(), "{id}: {:?}", resolved.warnings);
    }
}

#[test]
fn preset_ids_are_unique_and_kebab_case() {
    let mut seen = Vec::new();
    for (id, _) in PRESETS {
        assert!(id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'), "{id}");
        assert!(!seen.contains(id), "{id}");
        seen.push(id);
    }
}

#[test]
fn every_preset_sets_the_whole_terminal() {
    for (id, text) in PRESETS {
        let file = crate::theme::file::parse(text).expect(id);
        if *id == "cornercase-dark" {
            for key in crate::theme::tokens::TERMINAL_KEYS.iter().filter(|key| **key != "cursor") {
                assert!(file.terminal.contains_key(*key), "{id}: {key}");
            }
        }
    }
}

#[test]
fn light_presets_are_light_and_dark_ones_dark() {
    for (id, _) in PRESETS {
        let theme = resolve(id, &Presets).expect(id).theme;
        let light = id.ends_with("light") || id.ends_with("latte");
        assert_eq!(theme.is_light(), light, "{id}");
    }
}
