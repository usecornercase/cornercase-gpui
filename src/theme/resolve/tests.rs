use std::collections::BTreeMap;

use super::*;
use crate::theme::colour::{hex, rgb_bytes};
use crate::theme::presets;

struct Files(BTreeMap<String, String>);

impl Lookup for Files {
    fn text(&self, id: &str) -> Option<String> {
        presets::text(id).map(str::to_string).or_else(|| self.0.get(id).cloned())
    }
}

fn files(themes: &[(&str, &str)]) -> Files {
    Files(themes.iter().map(|(id, text)| ((*id).to_string(), (*text).to_string())).collect())
}

fn close(a: Rgba, b: [u8; 3]) -> bool {
    rgb_bytes(a).iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 1)
}

fn dark() -> Theme {
    resolve(ROOT, &files(&[])).expect("root").theme
}

#[test]
fn cornercase_dark_draws_today_s_colours() {
    let theme = dark();
    let chrome = theme.chrome;
    let today: [(Rgba, [u8; 3]); 14] = [
        (chrome.brand, [0x00, 0x5f, 0xff]),
        (chrome.brand_hover, [0x00, 0x50, 0xd8]),
        (chrome.on_brand, [0xf7, 0xf8, 0xfa]),
        (chrome.accent, [0x74, 0xa3, 0xff]),
        (chrome.glass, [0x0b, 0x0c, 0x0e]),
        (chrome.surface, [0x2a, 0x2e, 0x35]),
        (chrome.raised, [0x1d, 0x20, 0x25]),
        (chrome.text, [0xec, 0xee, 0xf1]),
        (chrome.soft, [0xb6, 0xbc, 0xc4]),
        (chrome.muted, [0x8b, 0x92, 0x9b]),
        (chrome.faint, [0x6b, 0x72, 0x7c]),
        (chrome.working, [0xf5, 0xd7, 0x6e]),
        (chrome.merged, [0x87, 0x5f, 0xff]),
        (chrome.failed, [0xff, 0x6b, 0x8b]),
    ];
    for (colour, bytes) in today {
        assert!(close(colour, bytes), "{} vs {bytes:?}", hex(colour, true));
    }
    assert!((chrome.accent_soft.a - 46. / 255.).abs() < 1e-3);
    assert!((chrome.selection.a - 71. / 255.).abs() < 1e-3);
    assert_eq!(theme.terminal.background, [26, 27, 38]);
    assert_eq!(theme.terminal.cursor, theme.terminal.foreground);
}

#[test]
fn a_theme_of_three_lines_changes_only_what_it_lists() {
    let mine = files(&[("mine", r##"{"chrome": {"text": "#ff0000"}, "opacity": {"cards": 0.5}}"##)]);
    let theme = resolve("mine", &mine).expect("mine").theme;
    assert_eq!(rgb_bytes(theme.chrome.text), [255, 0, 0]);
    assert_eq!(theme.chrome.soft, dark().chrome.soft);
    assert!((theme.opacity.cards - 0.5).abs() < f32::EPSILON);
}

#[test]
fn a_theme_extends_cornercase_dark_when_it_names_no_base() {
    let mine = files(&[("mine", r#"{"name": "Mine"}"#)]);
    let resolved = resolve("mine", &mine).expect("mine");
    assert_eq!(resolved.theme.chrome, dark().chrome);
    assert_eq!(resolved.theme.name, "Mine");
}

#[test]
fn a_chain_of_user_themes_applies_from_the_root_down() {
    let chain = files(&[
        ("a", r##"{"extends": "b", "chrome": {"soft": "#010101"}}"##),
        ("b", r##"{"extends": "c", "chrome": {"soft": "#020202", "muted": "#030303"}}"##),
        ("c", r##"{"chrome": {"soft": "#040404", "faint": "#050505"}}"##),
    ]);
    let theme = resolve("a", &chain).expect("a").theme;
    assert_eq!(rgb_bytes(theme.chrome.soft), [1, 1, 1]);
    assert_eq!(rgb_bytes(theme.chrome.muted), [3, 3, 3]);
    assert_eq!(rgb_bytes(theme.chrome.faint), [5, 5, 5]);
}

#[test]
fn a_theme_that_extends_itself_is_refused_with_its_names() {
    let looped = files(&[("a", r#"{"extends": "b"}"#), ("b", r#"{"extends": "a"}"#)]);
    let error = format!("{:#}", resolve("a", &looped).err().expect("refused"));
    assert!(error.contains("a → b → a"), "{error}");
}

#[test]
fn an_unknown_base_is_refused() {
    let lost = files(&[("a", r#"{"extends": "nowhere"}"#)]);
    let error = format!("{:#}", resolve("a", &lost).err().expect("refused"));
    assert!(error.contains("nowhere"), "{error}");
}

#[test]
fn a_chain_deeper_than_eight_is_refused() {
    let deep: Vec<(String, String)> =
        (0..10).map(|i| (format!("t{i}"), format!(r#"{{"extends": "t{}"}}"#, i + 1))).collect();
    let refs: Vec<(&str, &str)> = deep.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    assert!(resolve("t0", &files(&refs)).is_err());
}

#[test]
fn colours_derived_from_brand_follow_a_new_brand() {
    let mine = files(&[("mine", r##"{"chrome": {"brand": "#ff0000"}}"##)]);
    let theme = resolve("mine", &mine).expect("mine").theme;
    assert!(close(theme.chrome.brand_hover, [216, 0, 0]));
    assert_eq!(rgb_bytes(theme.chrome.selection), [255, 0, 0]);
}

#[test]
fn a_derived_colour_given_by_the_theme_wins() {
    let mine = files(&[("mine", r##"{"chrome": {"brand": "#ff0000", "brand_hover": "#00ff00"}}"##)]);
    let theme = resolve("mine", &mine).expect("mine").theme;
    assert_eq!(rgb_bytes(theme.chrome.brand_hover), [0, 255, 0]);
}

#[test]
fn an_unknown_key_is_a_warning_not_a_refusal() {
    let mine = files(&[("mine", r##"{"later": 1, "chrome": {"sparkle": "#fff"}}"##)]);
    let resolved = resolve("mine", &mine).expect("mine");
    assert_eq!(resolved.warnings.len(), 2, "{:?}", resolved.warnings);
}

#[test]
fn a_bad_colour_keeps_the_base_s_value_and_warns() {
    let mine = files(&[("mine", r#"{"chrome": {"text": "red"}}"#)]);
    let resolved = resolve("mine", &mine).expect("mine");
    assert_eq!(resolved.theme.chrome.text, dark().chrome.text);
    assert_eq!(resolved.warnings.len(), 1);
}

#[test]
fn an_alpha_on_a_terminal_colour_is_dropped_with_a_warning() {
    let mine = files(&[("mine", r##"{"terminal": {"red": "#ff000080"}}"##)]);
    let resolved = resolve("mine", &mine).expect("mine");
    assert_eq!(resolved.theme.terminal.ansi[1], [255, 0, 0]);
    assert_eq!(resolved.warnings.len(), 1);
}

#[test]
fn opacities_are_kept_within_their_limits() {
    let mine = files(&[("mine", r#"{"opacity": {"ground": 0.1, "cards": 5}}"#)]);
    let resolved = resolve("mine", &mine).expect("mine");
    assert!((resolved.theme.opacity.ground - 0.55).abs() < f32::EPSILON);
    assert!((resolved.theme.opacity.cards - 0.8).abs() < f32::EPSILON);
    assert_eq!(resolved.warnings.len(), 2);
}

#[test]
fn the_cursor_follows_the_foreground_until_set() {
    let follows = files(&[("a", r##"{"terminal": {"foreground": "#102030"}}"##)]);
    assert_eq!(resolve("a", &follows).expect("a").theme.terminal.cursor, [0x10, 0x20, 0x30]);
    let set = files(&[("a", r##"{"terminal": {"foreground": "#102030", "cursor": "#ffffff"}}"##)]);
    assert_eq!(resolve("a", &set).expect("a").theme.terminal.cursor, [255, 255, 255]);
}
