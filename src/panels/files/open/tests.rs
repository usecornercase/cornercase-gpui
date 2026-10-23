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
fn finds_commands_then_bundles_in_order() {
    let root = temp("find");
    let bin = root.join("bin");
    let apps = root.join("Applications");
    std::fs::create_dir_all(&bin).expect("the temporary files are writable");
    std::fs::create_dir_all(apps.join("Zed.app")).expect("the temporary files are writable");
    executable(&bin.join("open"));
    executable(&bin.join("code"));
    std::fs::write(bin.join("subl"), "").expect("the temporary files are writable");
    let found = find(&bin.to_string_lossy(), std::slice::from_ref(&apps));
    assert_eq!(
        found,
        vec![
            (Opener::Finder, Launch::Open),
            (Opener::VsCode, Launch::Command(bin.join("code"))),
            (Opener::Zed, Launch::Bundle(apps.join("Zed.app"))),
            (Opener::DefaultApp, Launch::Open),
        ]
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn argv_per_opener() {
    let file = Path::new("/w/src/main.rs");
    let code = Launch::Command(PathBuf::from("/bin/code"));
    assert_eq!(
        argv(Opener::VsCode, &code, file, Some(12), false).expect("an argv"),
        vec!["/bin/code", "-g", "/w/src/main.rs:12"]
    );
    assert_eq!(argv(Opener::VsCode, &code, Path::new("/w"), Some(3), true).expect("an argv"), vec!["/bin/code", "/w"]);
    let zed = Launch::Command(PathBuf::from("/bin/zed"));
    assert_eq!(argv(Opener::Zed, &zed, file, Some(2), false).expect("an argv"), vec!["/bin/zed", "/w/src/main.rs:2"]);
    let bundle = Launch::Bundle(PathBuf::from("/Applications/Zed.app"));
    assert_eq!(
        argv(Opener::Zed, &bundle, file, Some(2), false).expect("an argv"),
        vec!["open", "-a", "/Applications/Zed.app", "/w/src/main.rs"]
    );
    assert_eq!(
        argv(Opener::Finder, &Launch::Open, file, None, false).expect("an argv"),
        vec!["open", "-R", "/w/src/main.rs"]
    );
    assert_eq!(
        argv(Opener::Finder, &Launch::Open, Path::new("/w/src"), None, true).expect("an argv"),
        vec!["open", "/w/src"]
    );
    assert_eq!(
        argv(Opener::Finder, &Launch::Open, Path::new("/w/X.app"), None, true).expect("an argv"),
        vec!["open", "-R", "/w/X.app"]
    );
    assert!(argv(Opener::DefaultApp, &Launch::Open, Path::new("/w"), None, true).is_none());
}

#[test]
fn default_app_refuses_programs() {
    let root = temp("exec");
    let program = root.join("tool");
    executable(&program);
    assert!(argv(Opener::DefaultApp, &Launch::Open, &program, None, false).is_none());
    let plain = root.join("notes.txt");
    std::fs::write(&plain, "x").expect("the temporary files are writable");
    assert!(argv(Opener::DefaultApp, &Launch::Open, &plain, None, false).is_some());
    let _ = std::fs::remove_dir_all(&root);
}
