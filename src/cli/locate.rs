mod system;

use std::fmt;
use std::path::{Path, PathBuf};

use crate::desktop;
use crate::platform::Os;

pub(crate) use system::System;

pub(crate) const INSTALL_GUIDE: &str = "https://usecornercase.dev/docs/installation/";

const NAME: &str = "cornercase";
const MARK: &str = "cornercase-gpui-path-mark";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Version {
    major: u64,
    minor: u64,
    patch: u64,
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Origin {
    Variable,
    Chosen,
    Path,
    LoginShell,
    Installer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Problem {
    NotAProgram,
    DoesNotStart(String),
    NotCornercase(String),
    TooOld(Version),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Missing {
    NotFound { places: Vec<PathBuf>, asked_shell: bool },
    Unusable { path: PathBuf, origin: Origin, problem: Problem },
}

impl Missing {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::NotFound { .. } | Self::Unusable { problem: Problem::NotAProgram, .. } => "not found",
            Self::Unusable { problem: Problem::TooOld(_), .. } => "too old",
            Self::Unusable { problem: Problem::DoesNotStart(_), .. } => "does not start",
            Self::Unusable { problem: Problem::NotCornercase(_), .. } => "is not cornercase",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Binary {
    pub(crate) path: PathBuf,
    pub(crate) path_env: Option<String>,
}

impl Binary {
    pub(crate) fn plain(path: PathBuf) -> Self {
        Self { path, path_env: None }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Inputs {
    pub(crate) os: Os,
    pub(crate) variable: Option<String>,
    pub(crate) chosen: Option<PathBuf>,
    pub(crate) path: Vec<PathBuf>,
    pub(crate) shell: Option<PathBuf>,
    pub(crate) home: Option<PathBuf>,
    pub(crate) cargo_home: Option<PathBuf>,
}

impl Inputs {
    pub(crate) fn here(chosen: Option<String>) -> Self {
        let absolute = |name: &str| std::env::var_os(name).map(PathBuf::from).filter(|path| path.is_absolute());
        Self {
            os: Os::CURRENT,
            variable: std::env::var("CORNERCASE_BIN").ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty()),
            chosen: chosen.filter(|path| !path.is_empty()).map(PathBuf::from),
            path: desktop::path_dirs(),
            shell: absolute("SHELL"),
            home: desktop::home(),
            cargo_home: absolute("CARGO_HOME"),
        }
    }
}

pub(crate) trait Probe {
    fn find(&self, dirs: &[PathBuf], name: &str) -> Option<PathBuf>;
    fn is_program(&self, path: &Path) -> bool;
    fn version(&self, path: &Path, path_env: Option<&str>) -> Result<Vec<String>, String>;
    fn login_path(&self, shell: &Path) -> Option<String>;
}

struct Found {
    path: PathBuf,
    origin: Origin,
    path_env: Option<String>,
}

impl Found {
    fn plain(path: PathBuf, origin: Origin) -> Self {
        Self { path, origin, path_env: None }
    }
}

pub(crate) const fn minimum(os: Os) -> Version {
    match os {
        Os::Windows => Version { major: 0, minor: 12, patch: 287 },
        Os::Mac | Os::Linux => Version { major: 0, minor: 12, patch: 234 },
    }
}

impl Version {
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let numbers = text.trim().split(['-', '+']).next()?;
        let mut parts = numbers.trim().split('.').map(|part| part.parse::<u64>().ok());
        let version = Self { major: parts.next()??, minor: parts.next()??, patch: parts.next()?? };
        parts.next().is_none().then_some(version)
    }
}

pub(crate) fn parse_version(line: &str) -> Option<Version> {
    Version::parse(line.trim().strip_prefix("cornercase ")?)
}

fn verdict(answer: Result<Vec<String>, String>, os: Os) -> Result<(), Problem> {
    let lines = answer.map_err(Problem::DoesNotStart)?;
    let first = lines.first().map(|line| line.trim()).unwrap_or_default();
    let version = parse_version(first).ok_or_else(|| Problem::NotCornercase(first.chars().take(80).collect()))?;
    if version < minimum(os) { Err(Problem::TooOld(version)) } else { Ok(()) }
}

pub(crate) fn installer_dirs(os: Os, home: Option<&Path>, cargo_home: Option<&Path>) -> Vec<PathBuf> {
    let under = |relative: &str| home.map(|home| home.join(relative));
    let cargo = cargo_home.map(|cargo| cargo.join("bin")).or_else(|| under(".cargo/bin"));
    let own = [under(".local/bin"), cargo];
    let system: &[&str] = match os {
        Os::Mac => &["/opt/homebrew/bin", "/usr/local/bin"],
        Os::Linux => &["/home/linuxbrew/.linuxbrew/bin", "~/.linuxbrew/bin", "/usr/local/bin"],
        Os::Windows => &[],
    };
    let system = system.iter().map(|dir| match dir.strip_prefix("~/") {
        Some(relative) => under(relative),
        None => Some(PathBuf::from(dir)),
    });
    own.into_iter().chain(system).flatten().collect()
}

pub(crate) const fn asks_login_shell(os: Os) -> bool {
    !matches!(os, Os::Windows)
}

fn shell_args(shell: &Path) -> Vec<String> {
    let name = shell.file_name().and_then(|name| name.to_str()).unwrap_or_default();
    let flags: &[&str] = if matches!(name, "csh" | "tcsh") { &["-i", "-c"] } else { &["-l", "-i", "-c"] };
    let script = format!("printf '%s\\n' {MARK} \"$PATH\" {MARK}");
    flags.iter().map(|flag| (*flag).to_string()).chain([script]).collect()
}

fn shell_path(lines: &[String]) -> Option<String> {
    let start = lines.iter().position(|line| line.trim() == MARK)?;
    let path = lines.get(start + 1)?;
    (lines.get(start + 2)?.trim() == MARK).then(|| path.trim().to_string())
}

fn closed(lines: &[String]) -> bool {
    lines.iter().filter(|line| line.trim() == MARK).count() >= 2
}

fn unix_dirs(path: &str) -> Vec<PathBuf> {
    path.split(':').filter(|dir| dir.starts_with('/')).map(PathBuf::from).collect()
}

fn merged(shell: &[PathBuf], process: &[PathBuf]) -> String {
    let mut dirs: Vec<&PathBuf> = Vec::new();
    for dir in shell.iter().chain(process) {
        if !dirs.contains(&dir) {
            dirs.push(dir);
        }
    }
    dirs.iter().map(|dir| dir.to_string_lossy()).collect::<Vec<_>>().join(":")
}

pub(crate) fn locate(inputs: &Inputs, probe: &impl Probe) -> Result<Binary, Missing> {
    if let Some(name) = &inputs.variable {
        return named(name, inputs, probe);
    }
    let by_hand = inputs.chosen.clone().filter(|path| probe.is_program(path));
    let gone = inputs.chosen.clone().filter(|_| by_hand.is_none());
    let by_hand = by_hand.map(|path| checked(Found::plain(path, Origin::Chosen), inputs.os, probe));
    if let Some(Ok(binary)) = by_hand {
        return Ok(binary);
    }
    let Some(found) = find_binary(inputs, probe) else {
        let stale = gone.map(|path| Missing::Unusable { path, origin: Origin::Chosen, problem: Problem::NotAProgram });
        return Err(by_hand.and_then(Result::err).or(stale).unwrap_or_else(|| not_found(inputs, probe)));
    };
    checked(found, inputs.os, probe)
}

pub(crate) fn chosen(path: PathBuf, os: Os, probe: &impl Probe) -> Result<Binary, Missing> {
    if !probe.is_program(&path) {
        return Err(Missing::Unusable { path, origin: Origin::Chosen, problem: Problem::NotAProgram });
    }
    checked(Found::plain(path, Origin::Chosen), os, probe)
}

fn named(name: &str, inputs: &Inputs, probe: &impl Probe) -> Result<Binary, Missing> {
    let path = if name.contains(['/', '\\']) {
        Some(PathBuf::from(name)).filter(|path| probe.is_program(path))
    } else {
        probe.find(&inputs.path, name)
    };
    match path {
        Some(path) => checked(Found::plain(path, Origin::Variable), inputs.os, probe),
        None => Err(Missing::Unusable {
            path: PathBuf::from(name),
            origin: Origin::Variable,
            problem: Problem::NotAProgram,
        }),
    }
}

fn find_binary(inputs: &Inputs, probe: &impl Probe) -> Option<Found> {
    probe
        .find(&inputs.path, NAME)
        .map(|path| Found::plain(path, Origin::Path))
        .or_else(|| through_login_shell(inputs, probe))
        .or_else(|| probe.find(&places(inputs), NAME).map(|path| Found::plain(path, Origin::Installer)))
}

fn places(inputs: &Inputs) -> Vec<PathBuf> {
    installer_dirs(inputs.os, inputs.home.as_deref(), inputs.cargo_home.as_deref())
}

fn login_shell<'a>(inputs: &'a Inputs, probe: &impl Probe) -> Option<&'a Path> {
    inputs.shell.as_deref().filter(|shell| asks_login_shell(inputs.os) && probe.is_program(shell))
}

fn through_login_shell(inputs: &Inputs, probe: &impl Probe) -> Option<Found> {
    let shell = login_shell(inputs, probe)?;
    let listed = unix_dirs(&probe.login_path(shell)?);
    let path = probe.find(&listed, NAME)?;
    Some(Found { path, origin: Origin::LoginShell, path_env: Some(merged(&listed, &inputs.path)) })
}

fn not_found(inputs: &Inputs, probe: &impl Probe) -> Missing {
    Missing::NotFound { places: places(inputs), asked_shell: login_shell(inputs, probe).is_some() }
}

fn checked(found: Found, os: Os, probe: &impl Probe) -> Result<Binary, Missing> {
    match verdict(probe.version(&found.path, found.path_env.as_deref()), os) {
        Ok(()) => Ok(Binary { path: found.path, path_env: found.path_env }),
        Err(problem) => Err(Missing::Unusable { path: found.path, origin: found.origin, problem }),
    }
}

#[cfg(test)]
mod tests;
