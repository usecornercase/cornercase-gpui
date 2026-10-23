//! Opening a file or folder in the apps of this computer: Finder, the editors found (VS Code,
//! Cursor, Zed, Sublime Text) and the default app. cornercase leaves this to each window on
//! purpose (`design/opening-in-other-apps.md`: "No request"), since only the window knows the
//! apps of the machine it runs on; detection and argv follow that design.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Opener {
    Finder,
    VsCode,
    Cursor,
    Zed,
    Sublime,
    DefaultApp,
}

impl Opener {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Finder => "Finder",
            Self::VsCode => "VS Code",
            Self::Cursor => "Cursor",
            Self::Zed => "Zed",
            Self::Sublime => "Sublime Text",
            Self::DefaultApp => "Default app",
        }
    }

    /// The command on the `PATH` and the bundle in an Applications folder.
    fn editor(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::VsCode => Some(("code", "Visual Studio Code.app")),
            Self::Cursor => Some(("cursor", "Cursor.app")),
            Self::Zed => Some(("zed", "Zed.app")),
            Self::Sublime => Some(("subl", "Sublime Text.app")),
            Self::Finder | Self::DefaultApp => None,
        }
    }
}

/// How an opener is run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Launch {
    /// `open` itself (Finder, default app).
    Open,
    /// The editor's command, which takes `FILE:LINE`.
    Command(PathBuf),
    /// The editor's bundle, run through `open -a`, which takes no line.
    Bundle(PathBuf),
}

/// The openers this computer has, in a fixed order: `path_env` is `$PATH`, `apps` the
/// Applications folders.
pub(super) fn find(path_env: &str, apps: &[PathBuf]) -> Vec<(Opener, Launch)> {
    let on_path = |name: &str| std::env::split_paths(path_env).map(|dir| dir.join(name)).find(|p| is_executable(p));
    let mut found = Vec::new();
    let has_open = on_path("open").is_some() || Path::new("/usr/bin/open").exists();
    if has_open {
        found.push((Opener::Finder, Launch::Open));
    }
    for opener in [Opener::VsCode, Opener::Cursor, Opener::Zed, Opener::Sublime] {
        let Some((command, bundle)) = opener.editor() else { continue };
        if let Some(path) = on_path(command) {
            found.push((opener, Launch::Command(path)));
        } else if has_open && let Some(app) = apps.iter().map(|dir| dir.join(bundle)).find(|p| p.is_dir()) {
            found.push((opener, Launch::Bundle(app)));
        }
    }
    if has_open {
        found.push((Opener::DefaultApp, Launch::Open));
    }
    found
}

/// The openers of this computer.
pub(super) fn detect() -> Vec<(Opener, Launch)> {
    let mut apps = vec![PathBuf::from("/Applications")];
    if let Some(home) = std::env::var_os("HOME") {
        apps.push(PathBuf::from(home).join("Applications"));
    }
    find(&std::env::var("PATH").unwrap_or_default(), &apps)
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// Bundle-like folders Finder reveals rather than opens (`open X.app` would launch it).
const BUNDLES: &[&str] = &["app", "xcodeproj", "xcworkspace", "framework", "bundle", "playground"];

/// The command line for opening `path` (absolute) with an opener, at a 1-based line for an
/// editor; `None` when the opener does not take this path (the default app for a folder or a
/// program).
pub(super) fn argv(
    opener: Opener,
    launch: &Launch,
    path: &Path,
    line: Option<usize>,
    dir: bool,
) -> Option<Vec<String>> {
    let path_s = path.to_string_lossy().into_owned();
    let open = || "open".to_string();
    Some(match (opener, launch) {
        (Opener::Finder, _) => {
            let bundle = dir && path.extension().and_then(|e| e.to_str()).is_some_and(|e| BUNDLES.contains(&e));
            if dir && !bundle { vec![open(), path_s] } else { vec![open(), "-R".into(), path_s] }
        }
        (Opener::DefaultApp, _) => {
            if dir || is_executable(path) {
                return None;
            }
            vec![open(), path_s]
        }
        (_, Launch::Bundle(app)) => vec![open(), "-a".into(), app.to_string_lossy().into_owned(), path_s],
        (_, Launch::Command(command)) => {
            let command = command.to_string_lossy().into_owned();
            let target = match line {
                Some(line) if !dir => format!("{path_s}:{line}"),
                _ => path_s,
            };
            match opener {
                Opener::VsCode | Opener::Cursor if !dir => vec![command, "-g".into(), target],
                _ => vec![command, target],
            }
        }
        (_, Launch::Open) => vec![open(), path_s],
    })
}

/// Runs a command line and waits for it; an error names what failed.
pub(super) fn run(argv: &[String]) -> Result<(), String> {
    let (program, args) = argv.split_first().ok_or("nothing to run")?;
    let status = std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() { Ok(()) } else { Err(status.to_string()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cc-gpui-open-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::write(path, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn finds_commands_then_bundles_in_order() {
        let root = temp("find");
        let bin = root.join("bin");
        let apps = root.join("Applications");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(apps.join("Zed.app")).unwrap();
        executable(&bin.join("open"));
        executable(&bin.join("code"));
        // Not executable: not an opener.
        std::fs::write(bin.join("subl"), "").unwrap();
        let found = find(&bin.to_string_lossy(), &[apps.clone()]);
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
            argv(Opener::VsCode, &code, file, Some(12), false).unwrap(),
            vec!["/bin/code", "-g", "/w/src/main.rs:12"]
        );
        assert_eq!(argv(Opener::VsCode, &code, Path::new("/w"), Some(3), true).unwrap(), vec!["/bin/code", "/w"]);
        let zed = Launch::Command(PathBuf::from("/bin/zed"));
        assert_eq!(argv(Opener::Zed, &zed, file, Some(2), false).unwrap(), vec!["/bin/zed", "/w/src/main.rs:2"]);
        let bundle = Launch::Bundle(PathBuf::from("/Applications/Zed.app"));
        assert_eq!(
            argv(Opener::Zed, &bundle, file, Some(2), false).unwrap(),
            vec!["open", "-a", "/Applications/Zed.app", "/w/src/main.rs"]
        );
        assert_eq!(
            argv(Opener::Finder, &Launch::Open, file, None, false).unwrap(),
            vec!["open", "-R", "/w/src/main.rs"]
        );
        assert_eq!(
            argv(Opener::Finder, &Launch::Open, Path::new("/w/src"), None, true).unwrap(),
            vec!["open", "/w/src"]
        );
        assert_eq!(
            argv(Opener::Finder, &Launch::Open, Path::new("/w/X.app"), None, true).unwrap(),
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
        std::fs::write(&plain, "x").unwrap();
        assert!(argv(Opener::DefaultApp, &Launch::Open, &plain, None, false).is_some());
        let _ = std::fs::remove_dir_all(&root);
    }
}
