use super::*;

#[test]
fn a_name_becomes_a_file_stem() {
    assert_eq!(slug("My Gruvbox!"), "my-gruvbox");
    assert_eq!(slug("  "), "theme");
}

#[test]
fn a_taken_stem_gets_a_number() {
    let taken = |stem: &str| stem == "mine" || stem == "mine-2";
    assert_eq!(free_stem("Mine", &taken), "mine-3");
}

#[test]
fn a_stem_is_never_a_preset_s_id() {
    assert_eq!(free_stem("Nord", &|_| false), "nord-2");
}

#[test]
fn themes_sit_beside_the_settings_file() {
    let folder = folder_of(Path::new("/tmp/x/settings.json"));
    assert_eq!(choice_path(&folder), Path::new("/tmp/x/theme.json"));
    assert_eq!(theme_path(&folder, "mine"), Path::new("/tmp/x/themes/mine.json"));
}

#[test]
fn only_json_files_are_themes() {
    assert_eq!(id_of(Path::new("/a/themes/x.json")), Some("x".to_string()));
    assert_eq!(id_of(Path::new("/a/themes/x.json.new")), None);
}
