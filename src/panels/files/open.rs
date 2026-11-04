use std::path::{Path, PathBuf};

use crate::desktop;
use crate::platform::Os;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Opener {
    FileManager,
    VsCode,
    Cursor,
    Zed,
    Sublime,
    DefaultApp,
}

impl Opener {
    pub(super) fn label(self, os: Os) -> &'static str {
        match self {
            Self::FileManager => desktop::file_manager(os),
            Self::VsCode => "VS Code",
            Self::Cursor => "Cursor",
            Self::Zed => "Zed",
            Self::Sublime => "Sublime Text",
            Self::DefaultApp => "Default app",
        }
    }

    fn editor(self) -> Option<(&'static [&'static str], &'static str)> {
        match self {
            Self::VsCode => Some((&["code", "com.visualstudio.code"], "Visual Studio Code.app")),
            Self::Cursor => Some((&["cursor"], "Cursor.app")),
            Self::Zed => Some((&["zed", "zeditor", "dev.zed.Zed"], "Zed.app")),
            Self::Sublime => Some((&["subl", "com.sublimetext.three"], "Sublime Text.app")),
            Self::FileManager | Self::DefaultApp => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Launch {
    Desktop,
    Command(PathBuf),
    Bundle(PathBuf),
}

pub(super) fn find(os: Os, dirs: &[PathBuf], apps: &[PathBuf]) -> Vec<(Opener, Launch)> {
    let on = |name: &str| desktop::find_in(dirs, name);
    let mut found = Vec::new();
    if desktop::reveals(os, &on) {
        found.push((Opener::FileManager, Launch::Desktop));
    }
    let opens = desktop::opens(os, &on);
    for opener in [Opener::VsCode, Opener::Cursor, Opener::Zed, Opener::Sublime] {
        let Some((commands, bundle)) = opener.editor() else { continue };
        if let Some(path) = commands.iter().find_map(|command| on(command)) {
            found.push((opener, Launch::Command(path)));
        } else if os == Os::Mac
            && let Some(app) = apps.iter().map(|dir| dir.join(bundle)).find(|p| p.is_dir())
        {
            found.push((opener, Launch::Bundle(app)));
        }
    }
    if opens {
        found.push((Opener::DefaultApp, Launch::Desktop));
    }
    found
}

pub(super) fn search_dirs(os: Os, path: &[PathBuf], home: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = path.to_vec();
    if os == Os::Linux {
        dirs.push(PathBuf::from("/snap/bin"));
        dirs.extend(home.map(|home| home.join(".local/share/flatpak/exports/bin")));
        dirs.push(PathBuf::from("/var/lib/flatpak/exports/bin"));
    }
    dirs
}

pub(super) fn app_folders(os: Os, home: Option<&Path>) -> Vec<PathBuf> {
    if os != Os::Mac {
        return Vec::new();
    }
    std::iter::once(PathBuf::from("/Applications")).chain(home.map(|home| home.join("Applications"))).collect()
}

pub(super) fn detect() -> Vec<(Opener, Launch)> {
    let os = Os::CURRENT;
    let home = std::env::var_os("HOME").map(PathBuf::from);
    find(os, &search_dirs(os, &desktop::path_dirs(), home.as_deref()), &app_folders(os, home.as_deref()))
}

pub(super) fn default_app_opens(path: &Path, dir: bool) -> bool {
    !dir && !desktop::executable(path)
}

pub(super) fn argv(
    launch: &Launch,
    opener: Opener,
    path: &Path,
    line: Option<usize>,
    dir: bool,
) -> Option<Vec<String>> {
    let path_s = path.to_string_lossy().into_owned();
    match launch {
        Launch::Desktop => None,
        Launch::Bundle(app) => Some(vec!["open".into(), "-a".into(), app.to_string_lossy().into_owned(), path_s]),
        Launch::Command(command) => {
            let command = command.to_string_lossy().into_owned();
            let target = match line {
                Some(line) if !dir => format!("{path_s}:{line}"),
                _ => path_s,
            };
            Some(match opener {
                Opener::VsCode | Opener::Cursor if !dir => vec![command, "-g".into(), target],
                _ => vec![command, target],
            })
        }
    }
}

#[cfg(test)]
mod tests;
