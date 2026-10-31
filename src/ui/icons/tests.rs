use gpui::AssetSource as _;

use super::{Assets, Icon};

#[test]
fn every_icon_loads_as_an_svg() {
    for icon in Icon::ALL {
        let bytes = Assets.load(icon.path()).expect("loads").expect("is embedded");
        assert!(String::from_utf8_lossy(&bytes).contains("<svg"), "{icon:?}");
    }
}

#[test]
fn an_unknown_path_loads_nothing() {
    assert!(Assets.load("icons/nothing.svg").expect("loads").is_none());
}

#[test]
fn the_icons_folder_lists_every_icon() {
    assert_eq!(Assets.list("icons/").expect("lists").len(), Icon::ALL.len());
}

#[test]
fn every_icon_has_a_file_of_its_own() {
    let paths: std::collections::HashSet<&str> = Icon::ALL.into_iter().map(Icon::path).collect();
    assert_eq!(paths.len(), Icon::ALL.len());
}
