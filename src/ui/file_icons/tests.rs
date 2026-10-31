use super::*;

fn path_of(name: &str) -> &'static str {
    kind(name, false, false).path
}

#[test]
fn a_file_gets_the_icon_of_its_extension() {
    assert_eq!(path_of("main.rs"), "file-icons/rust.svg");
    assert_eq!(path_of("app.tsx"), "file-icons/react.svg");
    assert_eq!(path_of("data.json"), "file-icons/json.svg");
}

#[test]
fn a_known_name_wins_over_its_extension() {
    assert_eq!(path_of("Cargo.toml"), "file-icons/rust.svg");
    assert_eq!(path_of("other.toml"), "file-icons/toml.svg");
    assert_eq!(path_of("package.json"), "file-icons/npm.svg");
}

#[test]
fn names_and_extensions_ignore_case() {
    assert_eq!(path_of("MAIN.RS"), "file-icons/rust.svg");
    assert_eq!(path_of("readme.md"), "file-icons/info.svg");
}

#[test]
fn an_unknown_file_gets_the_generic_icon() {
    assert_eq!(path_of("notes.xyz"), "file-icons/default.svg");
    assert_eq!(path_of("Procfile"), "file-icons/default.svg");
}

#[test]
fn a_folder_shows_whether_it_is_open() {
    assert_eq!(kind("src", true, false).path, "file-icons/folder.svg");
    assert_eq!(kind("src", true, true).path, "file-icons/folder-open.svg");
    assert_eq!(kind("main.rs", true, false).path, "file-icons/folder.svg");
}

#[test]
fn one_file_can_take_two_tints() {
    assert_eq!(kind("a.c", false, false).tint, Tint::Blue);
    assert_eq!(kind("a.h", false, false).tint, Tint::Magenta);
    assert_eq!(kind("a.h", false, false).path, kind("a.c", false, false).path);
}

#[test]
fn every_file_icon_loads_as_an_svg() {
    for path in paths() {
        let bytes = load(path).expect("is embedded");
        assert!(String::from_utf8_lossy(bytes).contains("<svg"), "{path}");
    }
}

#[test]
fn every_file_in_the_folder_is_used() {
    let on_disk = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/file-icons"))
        .expect("lists the folder")
        .filter_map(|e| Some(e.ok()?.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "svg"))
        .count();
    assert_eq!(paths().len(), on_disk);
}
