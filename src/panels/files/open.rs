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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Launch {
    Open,
    Command(PathBuf),
    Bundle(PathBuf),
}

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

const BUNDLES: &[&str] = &["app", "xcodeproj", "xcworkspace", "framework", "bundle", "playground"];

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

pub(super) fn run(argv: &[String]) -> Result<(), String> {
    let (program, rest) = argv.split_first().ok_or("nothing to run")?;
    let status = std::process::Command::new(program)
        .args(rest)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() { Ok(()) } else { Err(status.to_string()) }
}

#[cfg(test)]
mod tests;
