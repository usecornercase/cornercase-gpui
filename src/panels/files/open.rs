use std::path::{Path, PathBuf};

use anyhow::Context as _;

use crate::desktop::{self, Lookup};
use crate::platform::Os;
use crate::store::Busy;

pub(crate) type Found = (Opener, Launch);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Opener {
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
pub(crate) enum Launch {
    Desktop,
    Command(PathBuf),
    Bundle(PathBuf),
}

#[derive(Clone)]
pub(crate) struct Opening {
    pub(crate) opener: Opener,
    pub(crate) launch: Launch,
    pub(crate) path: PathBuf,
    pub(crate) line: Option<usize>,
    pub(crate) dir: bool,
}

impl Opening {
    pub(crate) fn folder((opener, launch): &Found, path: &str) -> Self {
        Self { opener: *opener, launch: launch.clone(), path: PathBuf::from(path), line: None, dir: true }
    }

    pub(crate) fn busy(&self) -> Busy {
        Busy::new("open-with", format!("Opening in {}…", self.opener.label(Os::CURRENT)))
    }

    pub(crate) fn run(&self) -> anyhow::Result<()> {
        match self.opener {
            Opener::FileManager => desktop::reveal(&self.path, self.dir),
            Opener::DefaultApp => desktop::open(&self.path),
            _ => {
                let argv = argv(&self.launch, self.opener, &self.path, self.line.map(|l| l + 1), self.dir);
                let label = self.opener.label(Os::CURRENT);
                desktop::run(&argv.context("nothing to run")?).with_context(|| format!("could not open in {label}"))
            }
        }
    }
}

pub(crate) fn opener_label(opener: Opener, dir: bool) -> String {
    match opener {
        Opener::FileManager => desktop::show_label(Os::CURRENT, dir),
        Opener::DefaultApp => "Open with default app".to_string(),
        other => format!("Open in {}", other.label(Os::CURRENT)),
    }
}

pub(crate) fn for_folders(found: &[Found]) -> impl Iterator<Item = &Found> {
    found.iter().filter(|(opener, _)| *opener != Opener::DefaultApp)
}

pub(super) fn find(lookup: &Lookup, path: &[PathBuf], extra: &[PathBuf], apps: &[PathBuf]) -> Vec<Found> {
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

pub(crate) fn detect() -> Vec<Found> {
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
