use super::*;

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cc-gpui-open-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the temporary files are writable");
    dir
}

fn executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::write(path, "#!/bin/sh\n").expect("the temporary files are writable");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("the temporary files are writable");
}

#[test]
fn on_a_mac_finds_commands_then_bundles_in_order() {
    let root = temp("find");
    let bin = root.join("bin");
    let apps = root.join("Applications");
    std::fs::create_dir_all(&bin).expect("the temporary files are writable");
    std::fs::create_dir_all(apps.join("Zed.app")).expect("the temporary files are writable");
    executable(&bin.join("code"));
    std::fs::write(bin.join("subl"), "").expect("the temporary files are writable");
    let found = find(Os::Mac, std::slice::from_ref(&bin), &[], std::slice::from_ref(&apps));
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
        find(Os::Linux, std::slice::from_ref(&bin), std::slice::from_ref(&flatpak), std::slice::from_ref(&apps));
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
    assert_eq!(find(Os::Linux, std::slice::from_ref(&root), &[], &[]), vec![(Opener::FileManager, Launch::Desktop)]);
    assert_eq!(find(Os::Linux, &[root.join("nothing")], &[], &[]), vec![]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn on_linux_gio_alone_shows_in_the_file_manager_and_opens_with_the_default_app() {
    let root = temp("gio");
    executable(&root.join("gio"));
    let found = find(Os::Linux, std::slice::from_ref(&root), &[], &[]);
    assert_eq!(found, vec![(Opener::FileManager, Launch::Desktop), (Opener::DefaultApp, Launch::Desktop)]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_desktop_tools_count_only_on_the_path_where_they_are_run_from() {
    let root = temp("snap-tools");
    for tool in ["xdg-open", "gio", "gdbus", "dbus-send", "code"] {
        executable(&root.join(tool));
    }
    let found = find(Os::Linux, &[], std::slice::from_ref(&root), &[]);
    assert_eq!(found, vec![(Opener::VsCode, Launch::Command(root.join("code")))]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn linux_also_looks_for_editors_in_snap_and_flatpak_folders() {
    let dirs = extra_dirs(Os::Linux, Some(Path::new("/home/me")));
    assert_eq!(
        dirs,
        [
            PathBuf::from("/snap/bin"),
            "/home/me/.local/share/flatpak/exports/bin".into(),
            "/var/lib/flatpak/exports/bin".into()
        ]
    );
    assert_eq!(extra_dirs(Os::Mac, Some(Path::new("/Users/me"))), Vec::<PathBuf>::new());
}

#[test]
fn only_a_mac_has_applications_folders() {
    let home = Some(Path::new("/Users/me"));
    assert_eq!(app_folders(Os::Mac, home), [PathBuf::from("/Applications"), "/Users/me/Applications".into()]);
    assert_eq!(app_folders(Os::Linux, home), Vec::<PathBuf>::new());
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
fn the_file_manager_is_named_per_platform() {
    assert_eq!(Opener::FileManager.label(Os::Mac), "Finder");
    assert_eq!(Opener::FileManager.label(Os::Linux), "file manager");
}

#[test]
fn default_app_refuses_folders_and_programs() {
    let root = temp("exec");
    let program = root.join("tool");
    executable(&program);
    assert!(!default_app_opens(&program, false));
    assert!(!default_app_opens(&root, true));
    let plain = root.join("notes.txt");
    std::fs::write(&plain, "x").expect("the temporary files are writable");
    assert!(default_app_opens(&plain, false));
    let _ = std::fs::remove_dir_all(&root);
}
