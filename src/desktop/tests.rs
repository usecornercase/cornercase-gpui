use std::path::{Path, PathBuf};

use super::*;
use crate::test_util::in_usr_bin as only;

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cc-gpui-desktop-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the temporary files are writable");
    dir
}

fn files(dir: &Path, names: &[&str]) {
    for name in names {
        std::fs::write(dir.join(name), "").expect("the temporary files are writable");
    }
}

#[test]
fn the_machine_is_named_per_platform() {
    assert_eq!(format!("not on this {}", machine(Os::Mac)), "not on this Mac");
    assert_eq!(format!("not on this {}", machine(Os::Linux)), "not on this computer");
    assert_eq!(format!("not on this {}", machine(Os::Windows)), "not on this PC");
}

#[test]
fn the_file_manager_is_named_per_platform() {
    assert_eq!(show_label(Os::Mac, false), "Reveal in Finder");
    assert_eq!(show_label(Os::Mac, true), "Open in Finder");
    assert_eq!(show_label(Os::Linux, false), "Show in file manager");
    assert_eq!(show_label(Os::Linux, true), "Open in file manager");
    assert_eq!(show_label(Os::Windows, false), "Show in File Explorer");
    assert_eq!(show_label(Os::Windows, true), "Open in File Explorer");
    assert_eq!(shown_in(Os::Mac), "the Finder");
    assert_eq!(shown_in(Os::Windows), "File Explorer");
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
fn windows_opens_and_shows_through_its_shell_whatever_the_path_holds() {
    let none = only(&[]);
    assert!(opens(Os::Windows, &none));
    assert!(reveals(Os::Windows, &none));
    assert_eq!(open_argv(Os::Windows, Path::new(r"C:\w\a.pdf"), &none), None);
    assert_eq!(reveal_argv(Os::Windows, Path::new(r"C:\w\a.pdf"), false, &none), None);
    let opened = Shell::open(Path::new(r"C:\w\a & b %PATH%.pdf"));
    assert_eq!(opened, Shell { verb: None, file: r"C:\w\a & b %PATH%.pdf".into(), parameters: None });
}

#[test]
fn file_explorer_opens_a_folder_with_the_explore_verb() {
    let windows = Path::new(r"C:\Windows");
    let steps = explorer_steps(Path::new(r"C:\"), true, Some(windows));
    assert_eq!(steps, [Shell { verb: Some("explore"), file: r"C:\".into(), parameters: None }]);
}

#[test]
fn file_explorer_selects_a_file_with_the_systems_own_explorer_and_its_path_quoted() {
    let windows = PathBuf::from(r"C:\Windows");
    let file = Path::new(r"C:\w\it's, done.txt");
    let steps = explorer_steps(file, false, Some(&windows));
    let select = Shell {
        verb: None,
        file: windows.join("explorer.exe").into(),
        parameters: Some(r#"/select,"C:\w\it's, done.txt""#.into()),
    };
    assert_eq!(steps[0], select);
    assert_eq!(steps[1], Shell::explore(folder_of(file, false)));
    assert_eq!(steps.len(), 2);
    assert_eq!(explorer_steps(file, false, None), [Shell::explore(folder_of(file, false))]);
}

#[test]
fn a_verbatim_path_loses_its_prefix_for_the_shell() {
    assert_eq!(plain(Path::new(r"\\?\C:\w\a.rs")), PathBuf::from(r"C:\w\a.rs"));
    assert_eq!(plain(Path::new(r"\\?\UNC\host\share\a.rs")), PathBuf::from(r"\\host\share\a.rs"));
    assert_eq!(plain(Path::new(r"\\?\Volume{1}\a.rs")), PathBuf::from(r"\\?\Volume{1}\a.rs"));
    assert_eq!(plain(Path::new("/w/a.rs")), PathBuf::from("/w/a.rs"));
}

#[test]
fn finder_reveals_a_file_and_a_bundle_and_opens_a_folder() {
    let none = only(&[]);
    assert_eq!(reveal_argv(Os::Mac, Path::new("/w/a.rs"), false, &none).expect("an argv"), ["open", "-R", "/w/a.rs"]);
    assert_eq!(reveal_argv(Os::Mac, Path::new("/w/src"), true, &none).expect("an argv"), ["open", "/w/src"]);
    assert_eq!(reveal_argv(Os::Mac, Path::new("/w/X.app"), true, &none).expect("an argv"), ["open", "-R", "/w/X.app"]);
}

#[cfg(unix)]
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

#[cfg(unix)]
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

#[cfg(unix)]
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

#[cfg(unix)]
#[test]
fn data_dirs_follow_the_xdg_variables() {
    let home = Some(Path::new("/home/me"));
    assert_eq!(
        data_dirs(Os::Linux, home, None, None),
        [PathBuf::from("/home/me/.local/share"), "/usr/local/share".into(), "/usr/share".into()]
    );
    let dirs = data_dirs(Os::Linux, home, Some("/data".into()), Some("/nix/share:relative:/opt/share".into()));
    assert_eq!(dirs, [PathBuf::from("/data"), "/nix/share".into(), "/opt/share".into()]);
}

#[test]
fn windows_has_no_xdg_data_folders() {
    let home = Some(Path::new(r"C:\Users\me"));
    assert_eq!(data_dirs(Os::Windows, home, Some("/data".into()), Some("/share".into())), Vec::<PathBuf>::new());
}

#[test]
fn a_failure_says_the_tools_last_words() {
    let said = "\nxdg-open: no method available for opening '/w/a.x'\n\n";
    assert_eq!(failure("xdg-open", "exit status: 3", said), "xdg-open: no method available for opening '/w/a.x'");
    assert_eq!(failure("open", "exit status: 1", "  "), "open ended with exit status: 1");
}

#[cfg(unix)]
#[test]
fn a_tool_that_fails_reports_what_it_said() {
    let script = ["sh", "-c", "echo 'sh: it went wrong' >&2; exit 4"].map(String::from);
    let error = run(&script).expect_err("the script fails");
    assert_eq!(error.to_string(), "sh: it went wrong");
    assert!(run(&["true".to_string()]).is_ok());
    assert!(run(&["/nowhere/at-all".to_string()]).is_err());
}

#[cfg(windows)]
#[test]
fn a_windows_tool_that_fails_reports_what_it_said_or_its_exit_code() {
    let script = ["cmd", "/c", "echo cmd: it went wrong 1>&2 & exit 4"].map(String::from);
    assert_eq!(run(&script).expect_err("the script fails").to_string(), "cmd: it went wrong");
    let silent = ["cmd", "/c", "exit 3"].map(String::from);
    assert_eq!(run(&silent).expect_err("the script fails").to_string(), "cmd ended with exit code: 3");
    assert!(run(&["cmd", "/c", "exit 0"].map(String::from)).is_ok());
    assert!(run(&[r"C:\nowhere\at-all.exe".to_string()]).is_err());
}

#[test]
fn a_file_manager_call_waits_for_its_answer_on_linux_and_a_mac_launch_does_not() {
    assert_eq!(reveal_wait(Os::Mac), LAUNCH);
    assert_eq!(reveal_wait(Os::Linux), Wait { limit: ANSWER, late: Late::Failed });
    assert!(ANSWER > Duration::from_secs(25));
}

#[cfg(unix)]
#[test]
fn a_call_with_no_answer_in_time_fails_so_the_folder_can_open_instead() {
    let script = ["sh", "-c", "exec sleep 30"].map(String::from);
    let error = run_within(&script, Wait { limit: Duration::from_millis(50), late: Late::Failed }).expect_err("late");
    assert_eq!(error.to_string(), "sh did not answer within 0 s");
}

#[cfg(windows)]
#[test]
fn a_windows_call_with_no_answer_in_time_is_ended() {
    let pinging = ["ping", "-n", "30", "127.0.0.1"].map(String::from);
    let error = run_within(&pinging, Wait { limit: Duration::from_millis(50), late: Late::Failed }).expect_err("late");
    assert_eq!(error.to_string(), "ping did not answer within 0 s");
}

#[test]
fn windows_runs_only_what_pathext_lists_of_com_exe_bat_and_cmd_in_its_order() {
    assert_eq!(extensions(Os::Windows, Some(".COM;.EXE;.BAT;.CMD;.VBS;.JS;.PY")), [".com", ".exe", ".bat", ".cmd"]);
    assert_eq!(extensions(Os::Windows, Some(" .CMD ; .Exe;.vbs")), [".cmd", ".exe"]);
    assert_eq!(extensions(Os::Windows, Some(".VBS;.PY")), [".com", ".exe", ".bat", ".cmd"]);
    assert_eq!(extensions(Os::Windows, None), [".com", ".exe", ".bat", ".cmd"]);
    assert_eq!(extensions(Os::Linux, Some(".EXE")), Vec::<String>::new());
    assert_eq!(extensions(Os::Mac, None), Vec::<String>::new());
}

#[test]
fn on_windows_a_command_is_found_by_its_extensions_never_by_its_bare_name() {
    let root = temp("pathext");
    files(&root, &["code", "code.cmd", "zed.exe", "tool.vbs"]);
    let dirs = std::slice::from_ref(&root);
    let windows = Lookup::new(Os::Windows, Some(".COM;.EXE;.BAT;.CMD;.VBS"));
    assert_eq!(windows.find_in(dirs, "code"), Some(root.join("code.cmd")));
    assert_eq!(windows.find_in(dirs, "zed"), Some(root.join("zed.exe")));
    assert_eq!(windows.find_in(dirs, "zed.exe"), Some(root.join("zed.exe")));
    assert_eq!(windows.find_in(dirs, "tool"), None);
    assert_eq!(windows.find_in(&[PathBuf::from("relative")], "code"), None);
    assert!(windows.is_program(&root.join("code.cmd")));
    assert!(!windows.is_program(&root.join("code")));
    assert!(!windows.is_program(&root.join("tool.vbs")));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn on_windows_the_first_extension_pathext_lists_wins() {
    let root = temp("pathext-order");
    files(&root, &["both.bat", "both.exe"]);
    let dirs = std::slice::from_ref(&root);
    assert_eq!(Lookup::new(Os::Windows, None).find_in(dirs, "both"), Some(root.join("both.exe")));
    assert_eq!(Lookup::new(Os::Windows, Some(".BAT;.EXE")).find_in(dirs, "both"), Some(root.join("both.bat")));
    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn on_unix_only_files_with_the_execute_bit_are_found() {
    use std::os::unix::fs::PermissionsExt as _;

    let root = temp("bits");
    files(&root, &["plain", "tool", "tool.exe"]);
    std::fs::set_permissions(root.join("tool"), std::fs::Permissions::from_mode(0o755))
        .expect("the temporary files are writable");
    let dirs = std::slice::from_ref(&root);
    for os in [Os::Mac, Os::Linux] {
        let lookup = Lookup::new(os, Some(".EXE"));
        assert_eq!(lookup.find_in(dirs, "plain"), None);
        assert_eq!(lookup.find_in(dirs, "tool"), Some(root.join("tool")));
        assert_eq!(lookup.find_in(&[PathBuf::from("relative")], "sh"), None);
    }
    assert!(runs_when_opened(Os::Linux, &root.join("tool")));
    assert!(!runs_when_opened(Os::Linux, &root.join("tool.exe")));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn on_windows_what_runs_when_opened_is_known_by_its_real_extension() {
    let root = temp("runs");
    files(&root, &["notes.txt", "setup.EXE", "build.cmd", "run.ps1", "site.url", "link.lnk", "script.py", "readme"]);
    for program in ["setup.EXE", "build.cmd", "run.ps1", "site.url", "link.lnk", "script.py"] {
        assert!(runs_when_opened(Os::Windows, &root.join(program)), "{program}");
    }
    assert!(!runs_when_opened(Os::Windows, &root.join("notes.txt")));
    assert!(!runs_when_opened(Os::Windows, &root.join("readme")));
    assert!(runs_when_opened(Os::Windows, &root.join("gone.txt")));
    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(unix)]
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
fn a_scratch_folder_goes_when_dropped() {
    let scratch = Scratch::folder("gone").expect("the temporary folder is writable");
    let path = scratch.path().to_path_buf();
    std::fs::write(path.join("made"), "").expect("the folder is writable");
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

#[cfg(unix)]
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
    assert_eq!(selection_board(Os::Windows), Board::Clipboard);
}

#[test]
fn an_exe_is_a_program_on_windows_even_when_pathext_leaves_it_out() {
    let root = temp("exe-pathext");
    std::fs::create_dir_all(&root).expect("a folder");
    std::fs::write(root.join("cornercase.EXE"), "").expect("a file");
    let windows = Lookup::new(Os::Windows, Some(".CMD;.BAT"));
    assert!(windows.is_program(&root.join("cornercase.EXE")));
    assert_eq!(windows.find_in(std::slice::from_ref(&root), "cornercase"), None);
}
