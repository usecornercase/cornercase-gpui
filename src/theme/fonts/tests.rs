use super::{Fonts, choose};
use crate::platform::Os;

fn chosen(os: Os, installed: &[&str]) -> Fonts {
    let families: Vec<String> = installed.iter().map(ToString::to_string).collect();
    choose(os, &families, |name| installed.contains(&name))
}

#[test]
fn a_mac_keeps_menlo_and_the_system_font_whatever_is_installed() {
    let fonts = chosen(Os::Mac, &["DejaVu Sans Mono", "Cantarell"]);
    assert_eq!(fonts, Fonts { mono: "Menlo".into(), sans: ".SystemUIFont".into() });
}

#[test]
fn linux_takes_dejavu_sans_mono_first_as_menlo_grew_from_it() {
    let fonts = chosen(Os::Linux, &["JetBrains Mono", "Noto Sans Mono", "DejaVu Sans Mono"]);
    assert_eq!(fonts.mono, "DejaVu Sans Mono");
}

#[test]
fn linux_falls_back_along_its_list_of_monospace_fonts() {
    assert_eq!(chosen(Os::Linux, &["Liberation Mono", "Noto Sans Mono"]).mono, "Noto Sans Mono");
}

#[test]
fn linux_takes_any_installed_mono_family_when_none_of_its_list_is_there() {
    assert_eq!(chosen(Os::Linux, &["Cantarell", "Iosevka Term Mono"]).mono, "Iosevka Term Mono");
}

#[test]
fn a_family_named_mono_must_be_installed_to_be_taken() {
    let families = vec!["Ghost Mono".to_string()];
    let fonts = choose(Os::Linux, &families, |_| false);
    assert_eq!(fonts.mono, "DejaVu Sans Mono");
}

#[test]
fn a_word_that_only_contains_mono_is_not_a_monospace_family() {
    assert_eq!(chosen(Os::Linux, &["Monotype Corsiva"]).mono, "DejaVu Sans Mono");
}

#[test]
fn linux_takes_the_desktops_own_sans_before_the_common_ones() {
    assert_eq!(chosen(Os::Linux, &["DejaVu Sans", "Noto Sans", "Cantarell"]).sans, "Cantarell");
    assert_eq!(chosen(Os::Linux, &["Noto Sans", "Ubuntu Sans", "Adwaita Sans"]).sans, "Ubuntu Sans");
}

#[test]
fn linux_without_a_known_sans_leaves_it_to_gpuis_fallbacks() {
    assert_eq!(chosen(Os::Linux, &["Comic Neue"]).sans, ".SystemUIFont");
}
