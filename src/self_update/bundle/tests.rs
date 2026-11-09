use std::cell::RefCell;
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
fn an_old_app_names_the_window_that_set_it_aside() {
    assert_eq!(old_pid(OsStr::new(".cornercase-gpui-old-12-345")), Some(12));
    assert_eq!(old_pid(OsStr::new("cornercase.app")), None);
    assert_eq!(old_pid(OsStr::new(".cornercase-gpui-update-12")), None);
}

#[test]
fn an_old_app_is_kept_while_its_window_runs() {
    let name = OsStr::new(".cornercase-gpui-old-12-345");
    assert!(!is_free_old(name, |pid| pid == 12));
    assert!(is_free_old(name, |_| false));
    assert!(!is_free_old(OsStr::new("cornercase.app"), |_| false));
}

#[test]
fn only_a_pid_ps_does_not_find_counts_as_gone() {
    assert!(gone(Some(1)));
    for code in [Some(0), Some(2), None] {
        assert!(!gone(code), "{code:?}");
    }
}

struct Fake {
    signed: bool,
    done: RefCell<Vec<&'static str>>,
}

impl Fake {
    fn new(signed: bool) -> Self {
        Self { signed, done: RefCell::new(Vec::new()) }
    }
}

impl Steps for Fake {
    fn copy_out(&self, _: &[u8], stage: &Path) -> Result<PathBuf> {
        self.done.borrow_mut().push("copy");
        let app = stage.join("cornercase.app");
        fs::create_dir(&app)?;
        fs::write(app.join("Info.plist"), "new")?;
        Ok(app)
    }

    fn verify(&self, _: &Path) -> Result<()> {
        self.done.borrow_mut().push("verify");
        if self.signed { Ok(()) } else { anyhow::bail!("not signed") }
    }

    fn check_version(&self, _: &Path, _: &str) -> Result<()> {
        self.done.borrow_mut().push("version");
        Ok(())
    }
}

fn installed_app(scratch: &Scratch) -> PathBuf {
    let app = scratch.path().join("cornercase.app");
    fs::create_dir(&app).expect("the app");
    fs::write(app.join("Info.plist"), "old").expect("its plist");
    app
}

fn old_apps(folder: &Path) -> Vec<PathBuf> {
    let entries = fs::read_dir(folder).expect("the folder").flatten();
    entries.map(|entry| entry.path()).filter(|path| old_pid(path.file_name().unwrap_or_default()).is_some()).collect()
}

#[test]
fn an_app_without_the_teams_signature_is_not_installed() {
    let scratch = Scratch::folder("unsigned").expect("a folder");
    let app = installed_app(&scratch);
    let steps = Fake::new(false);
    assert!(install_with(&steps, b"dmg", &app, "0.2.0").is_err());
    assert_eq!(*steps.done.borrow(), ["copy", "verify"]);
    assert_eq!(fs::read_to_string(app.join("Info.plist")).expect("the old app"), "old");
    assert_eq!(old_apps(scratch.path()), Vec::<PathBuf>::new());
}

#[test]
fn a_signed_app_takes_the_old_ones_place_once_checked() {
    let scratch = Scratch::folder("signed").expect("a folder");
    let app = installed_app(&scratch);
    let steps = Fake::new(true);
    install_with(&steps, b"dmg", &app, "0.2.0").expect("installed");
    assert_eq!(*steps.done.borrow(), ["copy", "verify", "version"]);
    assert_eq!(fs::read_to_string(app.join("Info.plist")).expect("the new app"), "new");
    let aside = old_apps(scratch.path());
    assert_eq!(aside.len(), 1);
    assert_eq!(fs::read_to_string(aside[0].join("Info.plist")).expect("the old app"), "old");
}
