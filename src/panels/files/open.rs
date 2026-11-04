use std::path::{Path, PathBuf};

use crate::desktop::{self, Lookup};
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

pub(super) fn find(lookup: &Lookup, path: &[PathBuf], extra: &[PathBuf], apps: &[PathBuf]) -> Vec<(Opener, Launch)> {
    let os = lookup.os;
    let on_path = |name: &str| lookup.find_in(path, name);
    let on = |name: &str| on_path(name).or_else(|| lookup.find_in(extra, name));
    let mut found = Vec::new();
    if desktop::reveals(os, &on_path) {
        found.push((Opener::FileManager, Launch::Desktop));
    }
    let opens = desktop::opens(os, &on_path);
    for opener in [Opener::VsCode, Opener::Cursor, Opener::Zed, Opener::Sublime] {
        let Some((commands, bundle)) = opener.editor() else { continue };
        if let Some(command) = commands.iter().find_map(|command| on(command)) {
            found.push((opener, Launch::Command(command)));
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

pub(super) fn extra_dirs(os: Os, home: Option<&Path>, var: &dyn Fn(&str) -> Option<PathBuf>) -> Vec<PathBuf> {
    match os {
        Os::Mac => Vec::new(),
        Os::Linux => snaps_and_flatpaks(home),
        Os::Windows => install_folders(var),
    }
}

fn snaps_and_flatpaks(home: Option<&Path>) -> Vec<PathBuf> {
    let user = home.map(|home| home.join(".local/share/flatpak/exports/bin"));
    [Some(PathBuf::from("/snap/bin")), user, Some(PathBuf::from("/var/lib/flatpak/exports/bin"))]
        .into_iter()
        .flatten()
        .collect()
}

fn install_folders(var: &dyn Fn(&str) -> Option<PathBuf>) -> Vec<PathBuf> {
    let own = var("LOCALAPPDATA").map(|local| local.join("Programs"));
    let shared = var("ProgramFiles");
    let vs_code = [&own, &shared].into_iter().flatten().map(|dir| dir.join("Microsoft VS Code").join("bin"));
    let zed = own.iter().map(|dir| dir.join("Zed").join("bin"));
    let sublime = shared.iter().flat_map(|dir| [dir.join("Sublime Text"), dir.join("Sublime Text 3")]);
    vs_code.chain(zed).chain(sublime).collect()
}

pub(super) fn app_folders(os: Os, home: Option<&Path>) -> Vec<PathBuf> {
    if os != Os::Mac {
        return Vec::new();
    }
    std::iter::once(PathBuf::from("/Applications")).chain(home.map(|home| home.join("Applications"))).collect()
}

pub(super) fn detect() -> Vec<(Opener, Launch)> {
    let os = Os::CURRENT;
    let home = desktop::home();
    let var = |name: &str| std::env::var_os(name).map(PathBuf::from);
    let extra = extra_dirs(os, home.as_deref(), &var);
    find(&Lookup::here(), &desktop::path_dirs(), &extra, &app_folders(os, home.as_deref()))
}

pub(super) fn default_app_opens(os: Os, path: &Path, dir: bool) -> bool {
    !dir && !desktop::runs_when_opened(os, path)
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
