use std::fs;
use std::path::Path;

use super::*;
use crate::desktop::Scratch;

fn latest() -> Latest {
    let asset = |name: &str| Asset { name: name.to_string(), url: format!("https://example.test/{name}") };
    Latest {
        version: "0.2.0".into(),
        page: "https://example.test/release".into(),
        body: String::new(),
        assets: [
            "cornercase-gpui-universal-apple-darwin.dmg",
            "cornercase-gpui-x86_64-pc-windows-msvc.zip",
            "cornercase-gpui-x86_64-unknown-linux-gnu.tar.gz",
        ]
        .map(asset)
        .into(),
    }
}

fn exe() -> PathBuf {
    PathBuf::from("/apps/cornercase-gpui")
}

fn ways(released: bool, machine: (Os, &str), can_write: bool) -> Way {
    way(released, machine, Some(&exe()), &latest(), |_| can_write)
}

#[test]
fn a_downloaded_build_installs_itself_on_windows_and_linux() {
    for machine in [(Os::Windows, "x86_64"), (Os::Linux, "x86_64")] {
        assert!(matches!(ways(true, machine, true), Way::Replace { .. }), "{machine:?}");
    }
}

#[test]
fn a_bundle_on_macos_offers_the_download() {
    let Way::Download { url, why } = ways(true, (Os::Mac, "aarch64"), true) else { panic!("a download") };
    assert!(url.ends_with("apple-darwin.dmg"));
    assert_eq!(why, Why::Bundle);
}

#[test]
fn a_folder_that_cannot_be_written_offers_the_download() {
    let Way::Download { why, .. } = ways(true, (Os::Linux, "x86_64"), false) else { panic!("a download") };
    assert_eq!(why, Why::Folder(PathBuf::from("/apps")));
}

#[test]
fn a_machine_without_a_download_offers_the_release_page() {
    let Way::Download { url, why } = ways(true, (Os::Linux, "aarch64"), true) else { panic!("a download") };
    assert_eq!((url.as_str(), why), ("https://example.test/release", Why::NoDownload));
}

#[test]
fn a_program_without_a_known_place_offers_the_download() {
    let way = way(true, (Os::Linux, "x86_64"), None, &latest(), |_| true);
    assert!(matches!(way, Way::Download { why: Why::Unplaced, .. }));
}

#[test]
fn a_build_from_source_offers_the_command() {
    let Way::Command(command) = ways(false, (Os::Windows, "x86_64"), true) else { panic!("a command") };
    assert!(command.ends_with("--tag v0.2.0"));
}

#[test]
fn windows_unpacks_with_its_own_tar() {
    let windows = Path::new(r"C:\WINDOWS");
    let (program, args) = unpack_command(Os::Windows, Some(windows), Path::new("a.zip"), Path::new("out"));
    assert_eq!(program, windows.join("System32").join("tar.exe"));
    assert_eq!(args, ["-xf", "a.zip", "-C", "out"]);
}

#[test]
fn linux_unpacks_with_the_tar_on_the_path() {
    let (program, args) = unpack_command(Os::Linux, None, Path::new("a.tar.gz"), Path::new("out"));
    assert_eq!(program, Path::new("tar"));
    assert_eq!(args, ["-xzf", "a.tar.gz", "-C", "out"]);
}

#[test]
fn the_binary_is_found_at_the_root_or_one_folder_down() {
    let scratch = Scratch::folder("found").expect("a folder");
    let root = scratch.path();
    assert_eq!(find_binary(root, "cornercase-gpui"), None);
    fs::create_dir(root.join("inner")).expect("a folder");
    fs::write(root.join("inner").join("cornercase-gpui"), "x").expect("a file");
    assert_eq!(find_binary(root, "cornercase-gpui"), Some(root.join("inner").join("cornercase-gpui")));
    fs::write(root.join("cornercase-gpui"), "x").expect("a file");
    assert_eq!(find_binary(root, "cornercase-gpui"), Some(root.join("cornercase-gpui")));
}

#[test]
fn a_program_that_names_another_version_is_refused() {
    let line = |text: &str| vec![text.to_string()];
    assert!(verified(&line("cornercase-gpui 0.2.0"), "0.2.0").is_ok());
    assert!(verified(&line("cornercase-gpui 0.1.9"), "0.2.0").is_err());
    assert!(verified(&line("cornercase 0.2.0"), "0.2.0").is_err());
    assert!(verified(&[], "0.2.0").is_err());
}

fn pair(root: &Path) -> (PathBuf, PathBuf) {
    let (exe, new) = (root.join("cornercase-gpui.exe"), root.join("new.exe"));
    fs::write(&exe, "old").expect("a file");
    fs::write(&new, "new").expect("a file");
    (exe, new)
}

#[test]
fn the_new_binary_takes_the_old_ones_place() {
    let scratch = Scratch::folder("swap").expect("a folder");
    let (exe, new) = pair(scratch.path());
    swap(Os::Linux, &new, &exe).expect("swapped");
    assert_eq!(fs::read_to_string(&exe).expect("a file"), "new");
    assert!(!new.exists());
}

#[test]
fn windows_sets_the_old_binary_aside_first() {
    let scratch = Scratch::folder("aside").expect("a folder");
    let (exe, new) = pair(scratch.path());
    swap(Os::Windows, &new, &exe).expect("swapped");
    assert_eq!(fs::read_to_string(&exe).expect("a file"), "new");
    assert_eq!(fs::read_to_string(old_copy(&exe, 0)).expect("the old copy"), "old");
}

#[test]
fn the_old_binary_comes_back_when_the_new_one_cannot_take_its_place() {
    let scratch = Scratch::folder("back").expect("a folder");
    let (exe, new) = pair(scratch.path());
    fs::remove_file(&new).expect("removed");
    let why = swap(Os::Windows, &new, &exe).expect_err("a failure");
    assert!(format!("{why:#}").contains("the old one stays"));
    assert_eq!(fs::read_to_string(&exe).expect("the old binary"), "old");
}

#[test]
fn an_old_copy_takes_the_first_free_name() {
    let exe = PathBuf::from("/apps/cornercase-gpui.exe");
    assert_eq!(old_copy(&exe, 0), Path::new("/apps/cornercase-gpui.old.exe"));
    assert_eq!(old_copy(&exe, 2), Path::new("/apps/cornercase-gpui.old.2.exe"));
    let taken = [old_copy(&exe, 0), old_copy(&exe, 1)];
    assert_eq!(free_old_copy(&exe, |path| !taken.iter().any(|t| t == path)), Some(old_copy(&exe, 2)));
    assert_eq!(free_old_copy(&exe, |_| false), None);
}

#[test]
fn old_copies_are_told_from_other_files() {
    let exe = PathBuf::from("/apps/cornercase-gpui.exe");
    for name in ["cornercase-gpui.old.exe", "cornercase-gpui.old.7.exe"] {
        assert!(is_old_copy(OsStr::new(name), &exe), "{name}");
    }
    for name in ["cornercase-gpui.exe", "cornercase-gpui.old.exe.bak", "other.old.exe", "cornercase-gpui.old.x.exe"] {
        assert!(!is_old_copy(OsStr::new(name), &exe), "{name}");
    }
}

#[test]
fn old_copies_beside_the_program_are_removed() {
    let scratch = Scratch::folder("clean").expect("a folder");
    let (exe, _) = pair(scratch.path());
    fs::write(old_copy(&exe, 0), "x").expect("a file");
    fs::write(old_copy(&exe, 3), "x").expect("a file");
    remove_old_copies(&exe);
    assert!(exe.exists() && !old_copy(&exe, 0).exists() && !old_copy(&exe, 3).exists());
}
