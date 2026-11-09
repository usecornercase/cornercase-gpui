use std::fs;

use super::*;
use crate::desktop::Scratch;

fn words(args: &[OsString]) -> Vec<String> {
    args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect()
}

#[test]
fn the_app_is_found_from_its_program() {
    let exe = Path::new("/Applications/cornercase.app/Contents/MacOS/cornercase-gpui");
    assert_eq!(of(exe), Some(PathBuf::from("/Applications/cornercase.app")));
    assert_eq!(program_in(Path::new("/Applications/cornercase.app")), exe);
}

#[test]
fn a_program_outside_an_app_has_none() {
    for exe in
        ["/usr/local/bin/cornercase-gpui", "/apps/x/Contents/MacOS/cornercase-gpui", "/a.app/MacOS/cornercase-gpui"]
    {
        assert_eq!(of(Path::new(exe)), None, "{exe}");
    }
}

#[test]
fn the_download_opens_hidden_and_read_only() {
    let args = words(&attach_args(Path::new("/s/download.dmg"), Path::new("/s/mount")));
    assert_eq!(
        args,
        ["attach", "-nobrowse", "-readonly", "-noautoopen", "-noverify", "-mountpoint", "/s/mount", "/s/download.dmg"]
    );
    assert_eq!(words(&detach_args(Path::new("/s/mount"), false)), ["detach", "/s/mount"]);
    assert_eq!(words(&detach_args(Path::new("/s/mount"), true)), ["detach", "-force", "/s/mount"]);
}

#[test]
fn the_new_app_must_carry_the_teams_signature() {
    let args = words(&verify_args(Path::new("/s/cornercase.app")));
    assert_eq!(args[..3], ["--verify", "--deep", "--strict"]);
    assert!(args[3].starts_with("-R=anchor apple generic"));
    assert!(args[3].contains(r#"identifier "dev.usecornercase.gpui""#) && args[3].contains(r#"= "P7C3YXZT2N""#));
    assert_eq!(args[4], "/s/cornercase.app");
}

#[test]
fn the_app_is_the_folder_ending_in_app() {
    let scratch = Scratch::folder("mounted").expect("a folder");
    let root = scratch.path();
    assert_eq!(app_in(root), None);
    fs::write(root.join("README.app"), "x").expect("a file");
    fs::create_dir(root.join("Applications")).expect("a folder");
    assert_eq!(app_in(root), None);
    fs::create_dir(root.join("cornercase.app")).expect("a folder");
    assert_eq!(app_in(root), Some(root.join("cornercase.app")));
}

#[test]
fn the_restart_waits_for_this_window_then_opens_the_app() {
    let args = ["--tab".to_string(), "6".into()];
    let (program, line) = reopen(Path::new("/Applications/cornercase.app"), 42, &args);
    assert_eq!(program, Path::new("/bin/sh"));
    let line = words(&line);
    assert_eq!(line[0], "-c");
    assert!(line[1].contains(r#"kill -0 "$pid""#) && line[1].contains(r#"-n "$app" --args "$@""#));
    assert_eq!(line[2..], ["/usr/bin/open", "42", "/Applications/cornercase.app", "--tab", "6"]);
}

#[test]
fn old_apps_are_told_from_other_files() {
    assert!(is_old(OsStr::new(".cornercase-gpui-old-12-345")));
    assert!(!is_old(OsStr::new("cornercase.app")));
    assert!(!is_old(OsStr::new(".cornercase-gpui-update-12")));
}
