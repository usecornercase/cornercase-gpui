use super::{Fonts, choose, missing_mono};
use crate::platform::Os;

fn chosen(os: Os, installed: &[&str]) -> Fonts {
    let families: Vec<String> = installed.iter().map(ToString::to_string).collect();
    choose(os, &families, |name| installed.contains(&name), true)
}

fn alone(installed: &[&str]) -> Fonts {
    let families: Vec<String> = installed.iter().map(ToString::to_string).collect();
    choose(Os::Linux, &families, |name| installed.contains(&name), false)
}

#[test]
fn a_mac_keeps_menlo_and_the_system_font_whatever_is_installed() {
    let fonts = chosen(Os::Mac, &["DejaVu Sans Mono", "Cantarell"]);
    assert_eq!(fonts, Fonts { mono: "Menlo".into(), sans: ".SystemUIFont".into(), monospaced: true });
    assert_eq!(missing_mono(&fonts), None);
}

#[test]
fn linux_takes_dejavu_sans_mono_first_as_menlo_grew_from_it() {
    let fonts = chosen(Os::Linux, &["JetBrains Mono", "Noto Sans Mono", "DejaVu Sans Mono"]);
    assert_eq!(fonts.mono, "DejaVu Sans Mono");
    assert_eq!(missing_mono(&fonts), None);
}

#[test]
fn linux_falls_back_along_its_list_of_monospace_fonts() {
    assert_eq!(chosen(Os::Linux, &["Liberation Mono", "Noto Sans Mono"]).mono, "Noto Sans Mono");
}

#[test]
fn linux_takes_any_installed_mono_family_when_none_of_its_list_is_there() {
    assert_eq!(chosen(Os::Linux, &["Cantarell", "Iosevka Term Mono"]).mono, "Iosevka Term Mono");
    assert_eq!(chosen(Os::Linux, &["FreeSans", "FreeMono"]).mono, "FreeMono");
}

#[test]
fn a_family_named_mono_must_be_installed_to_be_taken() {
    let families = vec!["Ghost Mono".to_string()];
    assert_ne!(choose(Os::Linux, &families, |_| false, true).mono, "Ghost Mono");
}

#[test]
fn a_word_that_only_starts_with_mono_is_not_a_monospace_family() {
    assert_ne!(chosen(Os::Linux, &["Monotype Corsiva"]).mono, "Monotype Corsiva");
}

#[test]
fn gpuis_own_names_are_never_taken_for_installed_families() {
    assert_eq!(alone(&[".ZedMono", "Nimbus Mono PS"]).mono, "Nimbus Mono PS");
}

#[test]
fn without_a_monospace_font_the_terminal_takes_the_interfaces_font_and_says_so() {
    let fonts = chosen(Os::Linux, &["Noto Sans", "Monotype Corsiva"]);
    assert_eq!((fonts.mono.as_str(), fonts.sans.as_str(), fonts.monospaced), ("Noto Sans", "Noto Sans", false));
    assert!(missing_mono(&fonts).is_some_and(|note| note.contains("DejaVu Sans Mono")));
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

#[test]
fn where_none_of_gpuis_fallbacks_loads_only_installed_families_are_named() {
    let liberation = alone(&["Liberation Mono", "Liberation Sans"]);
    assert_eq!((liberation.mono.as_str(), liberation.sans.as_str()), ("Liberation Mono", "Liberation Sans"));
    let urw = alone(&["C059", "Nimbus Mono PS", "Nimbus Roman", "Nimbus Sans"]);
    assert_eq!((urw.mono.as_str(), urw.sans.as_str()), ("Nimbus Mono PS", "Nimbus Sans"));
    let free = alone(&["FreeMono", "FreeSans", "FreeSerif"]);
    assert_eq!((free.mono.as_str(), free.sans.as_str()), ("FreeMono", "FreeSans"));
}

#[test]
fn where_none_of_gpuis_fallbacks_loads_any_installed_family_draws_the_interface() {
    assert_eq!(alone(&["Arimo", "Cousine"]).sans, "Arimo");
    let mono_only = alone(&["Nimbus Mono PS"]);
    assert_eq!((mono_only.mono.as_str(), mono_only.sans.as_str()), ("Nimbus Mono PS", "Nimbus Mono PS"));
}
