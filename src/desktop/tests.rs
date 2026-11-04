use std::path::{Path, PathBuf};

use super::*;

fn only(tools: &'static [&'static str]) -> impl Fn(&str) -> Option<PathBuf> {
    move |name| tools.contains(&name).then(|| PathBuf::from("/usr/bin").join(name))
}

#[test]
fn the_machine_and_its_file_manager_are_named_per_platform() {
    assert_eq!(format!("not on this {}", machine(Os::Mac)), "not on this Mac");
    assert_eq!(format!("not on this {}", machine(Os::Linux)), "not on this computer");
    assert_eq!(show_label(Os::Mac, false), "Reveal in Finder");
    assert_eq!(show_label(Os::Mac, true), "Open in Finder");
    assert_eq!(show_label(Os::Linux, false), "Show in file manager");
    assert_eq!(show_label(Os::Linux, true), "Open in file manager");
}

#[test]
fn a_mac_opens_with_open() {
    let none = only(&[]);
    assert_eq!(open_argv(Os::Mac, Path::new("/w/a.pdf"), &none).expect("an argv"), ["open", "/w/a.pdf"]);
    assert!(opens(Os::Mac, &none));
}

#[test]
fn linux_opens_with_xdg_open_else_gio() {
    let file = Path::new("/w/a.pdf");
    let both = only(&["xdg-open", "gio"]);
    assert_eq!(open_argv(Os::Linux, file, &both).expect("an argv"), ["/usr/bin/xdg-open", "/w/a.pdf"]);
    let gio = only(&["gio"]);
    assert_eq!(open_argv(Os::Linux, file, &gio).expect("an argv"), ["/usr/bin/gio", "open", "/w/a.pdf"]);
    assert_eq!(open_argv(Os::Linux, file, &only(&[])), None);
    assert!(!opens(Os::Linux, &only(&["gdbus"])));
}

#[test]
fn finder_reveals_a_file_and_a_bundle_and_opens_a_folder() {
    let none = only(&[]);
    assert_eq!(reveal_argv(Os::Mac, Path::new("/w/a.rs"), false, &none).expect("an argv"), ["open", "-R", "/w/a.rs"]);
    assert_eq!(reveal_argv(Os::Mac, Path::new("/w/src"), true, &none).expect("an argv"), ["open", "/w/src"]);
    assert_eq!(reveal_argv(Os::Mac, Path::new("/w/X.app"), true, &none).expect("an argv"), ["open", "-R", "/w/X.app"]);
}

#[test]
fn linux_asks_the_file_manager_over_d_bus_to_show_an_item() {
    let argv = reveal_argv(Os::Linux, Path::new("/w/my file.rs"), false, &only(&["gdbus", "dbus-send"]));
    let expected = [
        "/usr/bin/gdbus",
        "call",
        "--session",
        "--dest",
        "org.freedesktop.FileManager1",
        "--object-path",
        "/org/freedesktop/FileManager1",
        "--method",
        "org.freedesktop.FileManager1.ShowItems",
        "['file:///w/my%20file.rs']",
        "''",
    ];
    assert_eq!(argv.expect("an argv"), expected);
}

#[test]
fn without_gdbus_dbus_send_opens_a_folder() {
    let argv = reveal_argv(Os::Linux, Path::new("/w/src"), true, &only(&["dbus-send"]));
    let expected = [
        "/usr/bin/dbus-send",
        "--session",
        "--print-reply",
        "--type=method_call",
        "--dest=org.freedesktop.FileManager1",
        "/org/freedesktop/FileManager1",
        "org.freedesktop.FileManager1.ShowFolders",
        "array:string:file:///w/src",
        "string:",
    ];
    assert_eq!(argv.expect("an argv"), expected);
    assert_eq!(reveal_argv(Os::Linux, Path::new("/w/src"), true, &only(&["xdg-open"])), None);
    assert!(reveals(Os::Linux, &only(&["dbus-send"])));
    assert!(!reveals(Os::Linux, &only(&[])));
}

#[test]
fn a_uri_cannot_break_out_of_its_quotes_or_split_into_two_items() {
    let uri = file_uri(Path::new("/w/it's, done\\.txt")).expect("an absolute path");
    assert_eq!(uri, "file:///w/it%27s%2C%20done%5C.txt");
    assert_eq!(file_uri(Path::new("relative")), None);
}

#[test]
fn the_folder_shown_when_the_file_manager_cannot_select_the_item() {
    assert_eq!(folder_of(Path::new("/w/src/a.rs"), false), Path::new("/w/src"));
    assert_eq!(folder_of(Path::new("/w/src"), true), Path::new("/w/src"));
}

#[test]
fn data_dirs_follow_the_xdg_variables() {
    let home = Some(Path::new("/home/me"));
    assert_eq!(
        data_dirs(home, None, None),
        [PathBuf::from("/home/me/.local/share"), "/usr/local/share".into(), "/usr/share".into()]
    );
    let dirs = data_dirs(home, Some("/data".into()), Some("/nix/share:relative:/opt/share".into()));
    assert_eq!(dirs, [PathBuf::from("/data"), "/nix/share".into(), "/opt/share".into()]);
}

#[test]
fn a_failure_says_the_tools_last_words() {
    let said = "\nxdg-open: no method available for opening '/w/a.x'\n\n";
    assert_eq!(failure("xdg-open", "exit status: 3", said), "xdg-open: no method available for opening '/w/a.x'");
    assert_eq!(failure("open", "exit status: 1", "  "), "open ended with exit status: 1");
}

#[test]
fn a_tool_that_fails_reports_what_it_said() {
    let script = ["sh", "-c", "echo 'sh: it went wrong' >&2; exit 4"].map(String::from);
    let error = run(&script).expect_err("the script fails");
    assert_eq!(error.to_string(), "sh: it went wrong");
    assert!(run(&["true".to_string()]).is_ok());
    assert!(run(&["/nowhere/at-all".to_string()]).is_err());
}

#[test]
fn only_executable_files_are_found() {
    let dir = std::env::temp_dir().join(format!("cc-gpui-desktop-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("the temporary files are writable");
    std::fs::write(dir.join("plain"), "").expect("the temporary files are writable");
    assert_eq!(find_in(std::slice::from_ref(&dir), "plain"), None);
    assert_eq!(find_in(&[PathBuf::from("relative")], "sh"), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_scratch_folder_is_private_and_goes_when_dropped() {
    use std::os::unix::fs::PermissionsExt as _;

    let scratch = Scratch::folder("test").expect("the temporary folder is writable");
    let path = scratch.path().to_path_buf();
    std::fs::write(path.join("made"), "").expect("the folder is writable");
    let mode = std::fs::metadata(&path).expect("the folder is there").permissions().mode();
    assert_eq!(mode & 0o777, 0o700);
    drop(scratch);
    assert!(!path.exists());
}

#[test]
fn a_scratch_name_someone_else_took_is_never_used() {
    let mut planted = None;
    let (path, ()) = fresh("test", |path| {
        if planted.is_none() {
            std::fs::create_dir(path)?;
            planted = Some(path.to_path_buf());
        }
        private_folder(path)
    })
    .expect("the temporary folder is writable");
    let planted = planted.expect("a folder was planted");
    assert_ne!(path, planted);
    let _ = std::fs::remove_dir(&path);
    let _ = std::fs::remove_dir(&planted);
}

#[test]
fn a_scratch_file_is_private_and_has_no_name() {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

    let file = scratch_file().expect("the temporary folder is writable");
    let meta = file.metadata().expect("the file is open");
    assert_eq!(meta.permissions().mode() & 0o777, 0o600);
    assert_eq!(meta.nlink(), 0);
}

#[test]
fn a_finished_selection_is_copied_on_a_mac_and_kept_in_the_primary_selection_on_linux() {
    assert_eq!(selection_board(Os::Mac), Board::Clipboard);
    assert_eq!(selection_board(Os::Linux), Board::Primary);
}
