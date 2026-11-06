use super::*;

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cc-gpui-open-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the temporary files are writable");
    dir
}

fn executable(path: &Path) {
    std::fs::write(path, "#!/bin/sh\n").expect("the temporary files are writable");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(path, mode).expect("the temporary files are writable");
    }
}

fn unix(os: Os) -> Lookup {
    Lookup::new(os, None)
}

#[test]
fn on_a_mac_finds_commands_then_bundles_in_order() {
    let root = temp("find");
    let bin = root.join("bin");
    let apps = root.join("Applications");
    std::fs::create_dir_all(&bin).expect("the temporary files are writable");
    std::fs::create_dir_all(apps.join("Zed.app")).expect("the temporary files are writable");
    executable(&bin.join("code"));
    let found = find(&unix(Os::Mac), std::slice::from_ref(&bin), &[], std::slice::from_ref(&apps));
    assert_eq!(
        found,
        vec![
            (Opener::FileManager, Launch::Desktop),
            (Opener::VsCode, Launch::Command(bin.join("code"))),
            (Opener::Zed, Launch::Bundle(apps.join("Zed.app"))),
            (Opener::DefaultApp, Launch::Desktop),
        ]
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn on_unix_a_command_without_the_execute_bit_is_not_an_editor() {
    let root = temp("bits");
    std::fs::write(root.join("subl"), "").expect("the temporary files are writable");
    for os in [Os::Mac, Os::Linux] {
        let found = find(&unix(os), std::slice::from_ref(&root), &[], &[]);
        assert!(found.iter().all(|(opener, _)| *opener != Opener::Sublime), "{found:?}");
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn on_linux_finds_editors_by_any_of_their_names_and_never_a_bundle() {
    let root = temp("linux");
    let (bin, flatpak, apps) = (root.join("bin"), root.join("flatpak"), root.join("Applications"));
    for dir in [&bin, &flatpak, &apps.join("Cursor.app")] {
        std::fs::create_dir_all(dir).expect("the temporary files are writable");
    }
    executable(&bin.join("xdg-open"));
    executable(&bin.join("zeditor"));
    executable(&flatpak.join("com.visualstudio.code"));
    let found =
        find(&unix(Os::Linux), std::slice::from_ref(&bin), std::slice::from_ref(&flatpak), std::slice::from_ref(&apps));
    assert_eq!(
        found,
        vec![
            (Opener::FileManager, Launch::Desktop),
            (Opener::VsCode, Launch::Command(flatpak.join("com.visualstudio.code"))),
            (Opener::Zed, Launch::Command(bin.join("zeditor"))),
            (Opener::DefaultApp, Launch::Desktop),
        ]
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn on_linux_without_xdg_open_there_is_no_default_app() {
    let root = temp("bare");
    executable(&root.join("dbus-send"));
    let linux = unix(Os::Linux);
    assert_eq!(find(&linux, std::slice::from_ref(&root), &[], &[]), vec![(Opener::FileManager, Launch::Desktop)]);
    assert_eq!(find(&linux, &[root.join("nothing")], &[], &[]), vec![]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn on_linux_gio_alone_shows_in_the_file_manager_and_opens_with_the_default_app() {
    let root = temp("gio");
    executable(&root.join("gio"));
    let found = find(&unix(Os::Linux), std::slice::from_ref(&root), &[], &[]);
    assert_eq!(found, vec![(Opener::FileManager, Launch::Desktop), (Opener::DefaultApp, Launch::Desktop)]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_desktop_tools_count_only_on_the_path_where_they_are_run_from() {
    let root = temp("snap-tools");
    for tool in ["xdg-open", "gio", "gdbus", "dbus-send", "code"] {
        executable(&root.join(tool));
    }
    let found = find(&unix(Os::Linux), &[], std::slice::from_ref(&root), &[]);
    assert_eq!(found, vec![(Opener::VsCode, Launch::Command(root.join("code")))]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn on_windows_editors_are_their_cmd_or_exe_and_the_shell_does_the_rest() {
    let root = temp("windows");
    let (bin, sublime, apps) = (root.join("bin"), root.join("Sublime Text"), root.join("Applications"));
    for dir in [&bin, &sublime, &apps.join("Cursor.app")] {
        std::fs::create_dir_all(dir).expect("the temporary files are writable");
    }
    for file in
        [bin.join("code"), bin.join("code.cmd"), bin.join("zed.exe"), bin.join("cursor.vbs"), sublime.join("subl.exe")]
    {
        std::fs::write(file, "").expect("the temporary files are writable");
    }
    let windows = Lookup::new(Os::Windows, Some(".COM;.EXE;.BAT;.CMD;.VBS"));
    let found = find(&windows, std::slice::from_ref(&bin), std::slice::from_ref(&sublime), std::slice::from_ref(&apps));
    assert_eq!(
        found,
        vec![
            (Opener::FileManager, Launch::Desktop),
            (Opener::VsCode, Launch::Command(bin.join("code.cmd"))),
            (Opener::Zed, Launch::Command(bin.join("zed.exe"))),
            (Opener::Sublime, Launch::Command(sublime.join("subl.exe"))),
            (Opener::DefaultApp, Launch::Desktop),
        ]
    );
    let bare = find(&windows, &[], &[], &[]);
    assert_eq!(bare, vec![(Opener::FileManager, Launch::Desktop), (Opener::DefaultApp, Launch::Desktop)]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn linux_also_looks_for_editors_in_snap_and_flatpak_folders() {
    let none = |_: &str| None;
    let dirs = extra_dirs(Os::Linux, Some(Path::new("/home/me")), &none);
    assert_eq!(
        dirs,
        [
            PathBuf::from("/snap/bin"),
            "/home/me/.local/share/flatpak/exports/bin".into(),
            "/var/lib/flatpak/exports/bin".into()
        ]
    );
    assert_eq!(extra_dirs(Os::Mac, Some(Path::new("/Users/me")), &none), Vec::<PathBuf>::new());
}

#[test]
fn windows_also_looks_where_the_editors_install_themselves() {
    let local = PathBuf::from(r"C:\Users\me\AppData\Local");
    let programs = PathBuf::from(r"C:\Program Files");
    let var = |name: &str| match name {
        "LOCALAPPDATA" => Some(local.clone()),
        "ProgramFiles" => Some(programs.clone()),
        _ => None,
    };
    let own = local.join("Programs");
    let expected = [
        own.join("Microsoft VS Code").join("bin"),
        programs.join("Microsoft VS Code").join("bin"),
        own.join("Zed").join("bin"),
        programs.join("Sublime Text"),
        programs.join("Sublime Text 3"),
    ];
    assert_eq!(extra_dirs(Os::Windows, Some(Path::new(r"C:\Users\me")), &var), expected);
    assert_eq!(extra_dirs(Os::Windows, None, &|_: &str| None), Vec::<PathBuf>::new());
}

#[test]
fn only_a_mac_has_applications_folders() {
    let home = Some(Path::new("/Users/me"));
    assert_eq!(app_folders(Os::Mac, home), [PathBuf::from("/Applications"), "/Users/me/Applications".into()]);
    assert_eq!(app_folders(Os::Linux, home), Vec::<PathBuf>::new());
    assert_eq!(app_folders(Os::Windows, Some(Path::new(r"C:\Users\me"))), Vec::<PathBuf>::new());
}

#[test]
fn argv_per_editor() {
    let file = Path::new("/w/src/main.rs");
    let code = Launch::Command(PathBuf::from("/bin/code"));
    assert_eq!(
        argv(&code, Opener::VsCode, file, Some(12), false).expect("an argv"),
        vec!["/bin/code", "-g", "/w/src/main.rs:12"]
    );
    assert_eq!(argv(&code, Opener::VsCode, Path::new("/w"), Some(3), true).expect("an argv"), vec!["/bin/code", "/w"]);
    let zed = Launch::Command(PathBuf::from("/bin/zeditor"));
    assert_eq!(
        argv(&zed, Opener::Zed, file, Some(2), false).expect("an argv"),
        vec!["/bin/zeditor", "/w/src/main.rs:2"]
    );
    let bundle = Launch::Bundle(PathBuf::from("/Applications/Zed.app"));
    assert_eq!(
        argv(&bundle, Opener::Zed, file, Some(2), false).expect("an argv"),
        vec!["open", "-a", "/Applications/Zed.app", "/w/src/main.rs"]
    );
    assert!(argv(&Launch::Desktop, Opener::FileManager, file, None, false).is_none());
}

#[test]
fn a_windows_editor_gets_the_path_and_line_as_they_are() {
    let code = Launch::Command(PathBuf::from(r"C:\Code\bin\code.cmd"));
    let file = Path::new(r"C:\w\a & b\main.rs");
    let expected = [r"C:\Code\bin\code.cmd", "-g", r"C:\w\a & b\main.rs:12"];
    assert_eq!(argv(&code, Opener::VsCode, file, Some(12), false).expect("an argv"), expected);
}

#[test]
fn the_file_manager_is_named_per_platform() {
    assert_eq!(Opener::FileManager.label(Os::Mac), "Finder");
    assert_eq!(Opener::FileManager.label(Os::Linux), "file manager");
    assert_eq!(Opener::FileManager.label(Os::Windows), "File Explorer");
}

#[cfg(unix)]
#[test]
fn on_unix_the_default_app_refuses_folders_and_programs() {
    let root = temp("exec");
    let program = root.join("tool");
    executable(&program);
    let plain = root.join("notes.txt");
    std::fs::write(&plain, "x").expect("the temporary files are writable");
    for os in [Os::Mac, Os::Linux] {
        assert!(!default_app_opens(os, &program, false));
        assert!(!default_app_opens(os, &root, true));
        assert!(default_app_opens(os, &plain, false));
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn on_windows_the_default_app_refuses_folders_programs_and_what_is_gone() {
    let root = temp("exec-windows");
    for name in ["notes.txt", "setup.exe", "build.BAT", "tool"] {
        std::fs::write(root.join(name), "x").expect("the temporary files are writable");
    }
    assert!(default_app_opens(Os::Windows, &root.join("notes.txt"), false));
    assert!(default_app_opens(Os::Windows, &root.join("tool"), false));
    assert!(!default_app_opens(Os::Windows, &root.join("setup.exe"), false));
    assert!(!default_app_opens(Os::Windows, &root.join("build.BAT"), false));
    assert!(!default_app_opens(Os::Windows, &root.join("gone.txt"), false));
    assert!(!default_app_opens(Os::Windows, &root, true));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_folder_is_offered_every_opener_but_the_default_app() {
    let found = [
        (Opener::FileManager, Launch::Desktop),
        (Opener::VsCode, Launch::Command(PathBuf::from("/bin/code"))),
        (Opener::DefaultApp, Launch::Desktop),
    ];
    let offered: Vec<Opener> = for_folders(&found).map(|(opener, _)| *opener).collect();
    assert_eq!(offered, [Opener::FileManager, Opener::VsCode]);
}

#[test]
fn opening_a_folder_names_no_line() {
    let found = (Opener::Zed, Launch::Command(PathBuf::from("/bin/zed")));
    let opening = Opening::folder(&found, "/work/shop");
    assert_eq!((opening.path, opening.line, opening.dir), (PathBuf::from("/work/shop"), None, true));
}
