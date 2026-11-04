use std::path::{Path, PathBuf};

use super::{Style, home_relative, on_a_drive, rooted};
use crate::platform::Os;

#[test]
fn windows_writes_its_paths_one_way_and_the_others_another() {
    assert_eq!(Style::of(Os::Windows), Style::Windows);
    assert_eq!(Style::of(Os::Mac), Style::Unix);
    assert_eq!(Style::of(Os::Linux), Style::Unix);
}

#[test]
fn a_path_with_a_drive_or_a_share_is_a_windows_path() {
    for path in [r"C:\Users\me", "c:/Users/me", r"\\server\share\x", r"\\?\C:\x", r"D:\"] {
        assert_eq!(Style::of_path(path), Style::Windows, "{path}");
    }
    for path in ["/home/me", "src/x.rs", r"src\x.rs", "C:", "Cx:/y", "//server/share", ""] {
        assert_eq!(Style::of_path(path), Style::Unix, "{path}");
    }
}

#[test]
fn a_backslash_separates_only_on_windows() {
    assert!(Style::Windows.is_separator('\\') && Style::Windows.is_separator('/'));
    assert!(Style::Unix.is_separator('/') && !Style::Unix.is_separator('\\'));
}

#[test]
fn an_absolute_path_is_judged_by_its_machine() {
    assert!(Style::Unix.is_absolute("/srv/x"));
    assert!(!Style::Unix.is_absolute(r"C:\x"));
    assert!(Style::Windows.is_absolute(r"C:\x") && Style::Windows.is_absolute("C:/x"));
    assert!(Style::Windows.is_absolute(r"\\server\share"));
    assert!(!Style::Windows.is_absolute("/srv/x") && !Style::Windows.is_absolute("C:x"));
}

#[test]
fn a_path_is_absolute_in_the_form_it_is_written_in() {
    assert!(rooted("/srv/x") && rooted(r"C:\x") && rooted("C:/x") && rooted(r"\\server\share\x"));
    assert!(!rooted("src/x.rs") && !rooted(r"src\x.rs") && !rooted("C:x") && !rooted(""));
}

#[test]
fn a_name_follows_the_last_separator_of_its_machine() {
    assert_eq!(Style::Unix.split("/srv/shop/"), ("/srv", "shop"));
    assert_eq!(Style::Unix.split(r"/srv/a\b"), ("/srv", r"a\b"));
    assert_eq!(Style::Windows.split(r"C:\Users\me\shop\"), (r"C:\Users\me", "shop"));
    assert_eq!(Style::Windows.split("C:/Users/me/x.rs"), ("C:/Users/me", "x.rs"));
    assert_eq!(Style::Windows.split(r"src\panels/x.rs"), (r"src\panels", "x.rs"));
}

#[test]
fn a_folder_at_the_root_keeps_its_root() {
    assert_eq!(Style::Unix.split("x.rs"), ("", "x.rs"));
    assert_eq!(Style::Unix.split("/x.rs"), ("/", "x.rs"));
    assert_eq!(Style::Windows.split(r"C:\x.rs"), (r"C:\", "x.rs"));
    assert_eq!(Style::Unix.name("/"), "");
    assert_eq!(Style::Windows.name_start(r"src\x.rs"), 4);
    assert_eq!(Style::Unix.name_start(r"src\x.rs"), 0);
}

#[test]
fn a_path_is_shown_with_its_machines_separator() {
    assert_eq!(Style::Windows.in_form("src/panels/x.rs"), r"src\panels\x.rs");
    assert_eq!(Style::Unix.in_form(r"src/a\b"), r"src/a\b");
    assert_eq!(Style::Windows.shown(Path::new("src/panels/x.rs")), r"src\panels\x.rs");
    assert_eq!(Style::Unix.shown(&Path::new("src").join("panels").join("x.rs")), "src/panels/x.rs");
    assert_eq!(Style::Unix.shown(Path::new("/srv/x")), "/srv/x");
    assert_eq!(Style::Unix.shown(Path::new("")), "");
}

#[test]
fn a_relative_path_from_windows_takes_this_panels_separator() {
    assert_eq!(Style::Windows.relative(r"src\panels\x.rs"), Path::new("src").join("panels").join("x.rs"));
    assert_eq!(Style::Unix.relative("src/x.rs"), PathBuf::from("src/x.rs"));
}

#[test]
fn a_relative_path_joins_its_root_with_the_roots_separator() {
    let windows = Style::Windows.join(Path::new(r"C:\Users\me\proj"), Path::new("src/x.rs"));
    assert_eq!(windows.to_string_lossy(), r"C:\Users\me\proj\src\x.rs");
    let unix = Style::Unix.join(Path::new("/home/me/proj"), &Path::new("src").join("x.rs"));
    assert_eq!(unix.to_string_lossy(), "/home/me/proj/src/x.rs");
    assert_eq!(Style::Windows.join(Path::new(r"C:\proj"), Path::new(r"D:\x.rs")), PathBuf::from(r"D:\x.rs"));
    assert_eq!(Style::Unix.join(Path::new("/proj"), Path::new("/etc/hosts")), PathBuf::from("/etc/hosts"));
    assert_eq!(Style::Unix.join(Path::new("/proj"), Path::new("")), PathBuf::from("/proj"));
}

#[test]
fn only_a_path_on_a_drive_is_on_a_drive() {
    assert!(on_a_drive(r"C:\Users\me\ding.wav") && on_a_drive("C:/x.wav") && on_a_drive(r"\\?\C:\x.wav"));
    assert!(!on_a_drive(r"\\server\share\x.wav") && !on_a_drive(r"\\?\UNC\server\x.wav"));
    assert!(!on_a_drive("/home/me/x.wav") && !on_a_drive("C:x.wav"));
}

#[test]
fn paths_under_home_start_with_a_tilde() {
    assert_eq!(home_relative("/Users/ana/src/x", "/Users/ana"), "~/src/x");
    assert_eq!(home_relative("/Users/ana", "/Users/ana/"), "~");
    assert_eq!(home_relative("/Users/anabel/x", "/Users/ana"), "/Users/anabel/x");
    assert_eq!(home_relative("/Users/Ana/x", "/Users/ana"), "/Users/Ana/x");
    assert_eq!(home_relative("/srv/x", ""), "/srv/x");
    assert_eq!(home_relative("/srv/x", "/"), "/srv/x");
}

#[test]
fn a_windows_home_matches_in_any_case_and_keeps_its_backslashes() {
    assert_eq!(home_relative(r"C:\Users\me\proj\src", r"C:\Users\me"), r"~\proj\src");
    assert_eq!(home_relative(r"c:\users\ME\proj", r"C:\Users\me"), r"~\proj");
    assert_eq!(home_relative("C:/Users/me/proj", r"C:\Users\me\"), "~/proj");
    assert_eq!(home_relative(r"C:\Users\me", r"C:\Users\me"), "~");
    assert_eq!(home_relative(r"C:\Users\meg\x", r"C:\Users\me"), r"C:\Users\meg\x");
    assert_eq!(home_relative(r"C:\Users\Émile\x", r"C:\Users\émile"), r"~\x");
    assert_eq!(home_relative("/home/me/proj", r"C:\Users\me"), "/home/me/proj");
}
