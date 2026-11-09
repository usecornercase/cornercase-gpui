use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context as _, Result};

use super::install::{self, Stage};

const REQUIREMENT: &str =
    r#"anchor apple generic and identifier "dev.usecornercase.gpui" and certificate leaf[subject.OU] = "P7C3YXZT2N""#;
const OLD_PREFIX: &str = ".cornercase-gpui-old-";
const HDIUTIL: &str = "/usr/bin/hdiutil";
const DITTO: &str = "/usr/bin/ditto";
const CODESIGN: &str = "/usr/bin/codesign";
const OPEN: &str = "/usr/bin/open";
const REOPEN: &str =
    r#"pid=$1 app=$2; shift 2; while kill -0 "$pid" 2>/dev/null; do sleep 0.1; done; exec "$0" -n "$app" --args "$@""#;

pub(crate) fn of(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let bundle = contents.parent()?;
    let named = |path: &Path, name: &str| path.file_name() == Some(OsStr::new(name));
    let app = bundle.extension() == Some(OsStr::new("app"));
    (named(macos, "MacOS") && named(contents, "Contents") && app).then(|| bundle.to_path_buf())
}

pub(crate) fn program_in(bundle: &Path) -> PathBuf {
    bundle.join("Contents").join("MacOS").join(install::PROGRAM)
}

pub(crate) fn is_old(file_name: &OsStr) -> bool {
    file_name.to_string_lossy().starts_with(OLD_PREFIX)
}

pub(super) fn install(image: &[u8], bundle: &Path, version: &str) -> Result<()> {
    let stage = Stage::beside(bundle)?;
    let new = copied_out(image, stage.path())?;
    install::run_tool(
        Path::new(CODESIGN),
        &verify_args(&new),
        "the downloaded app is not signed by cornercase's developer, so it was not installed",
    )?;
    install::check_binary(&program_in(&new), version)?;
    install::replace(&new, bundle, &aside(bundle)?)
}

fn copied_out(image: &[u8], stage: &Path) -> Result<PathBuf> {
    let dmg = stage.join("download.dmg");
    fs::write(&dmg, image).context("could not save the download")?;
    let mount = stage.join("mount");
    fs::create_dir(&mount).context("could not make a folder to open the download in")?;
    let opened = Mount::attach(&dmg, &mount)?;
    let app = app_in(&mount).context("the download holds no app")?;
    let new = stage.join(app.file_name().unwrap_or_default());
    install::run_tool(
        Path::new(DITTO),
        &[app.into(), new.clone().into()],
        "could not copy the app out of the download",
    )?;
    drop(opened);
    Ok(new)
}

pub(crate) fn app_in(folder: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(folder).ok()?;
    entries
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.is_dir() && path.extension() == Some(OsStr::new("app")))
}

fn aside(bundle: &Path) -> Result<PathBuf> {
    let folder = bundle.parent().context("the app has no folder")?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|time| time.as_nanos()).unwrap_or_default();
    Ok(folder.join(format!("{OLD_PREFIX}{}-{now}", std::process::id())))
}

pub(crate) fn attach_args(image: &Path, at: &Path) -> Vec<OsString> {
    let flags = ["attach", "-nobrowse", "-readonly", "-noautoopen", "-noverify", "-mountpoint"];
    flags.into_iter().map(OsString::from).chain([at.into(), image.into()]).collect()
}

pub(crate) fn detach_args(at: &Path, force: bool) -> Vec<OsString> {
    let force = force.then(|| OsString::from("-force"));
    [Some(OsString::from("detach")), force, Some(at.into())].into_iter().flatten().collect()
}

pub(crate) fn verify_args(app: &Path) -> Vec<OsString> {
    let flags = ["--verify".to_string(), "--deep".into(), "--strict".into(), format!("-R={REQUIREMENT}")];
    flags.into_iter().map(OsString::from).chain([app.into()]).collect()
}

pub(crate) fn reopen(bundle: &Path, pid: u32, args: &[String]) -> (PathBuf, Vec<OsString>) {
    let lead = ["-c", REOPEN, OPEN].map(OsString::from);
    let place = [OsString::from(pid.to_string()), bundle.into()];
    let args = args.iter().map(OsString::from);
    (PathBuf::from("/bin/sh"), lead.into_iter().chain(place).chain(args).collect())
}

struct Mount(PathBuf);

impl Mount {
    fn attach(image: &Path, at: &Path) -> Result<Self> {
        install::run_tool(Path::new(HDIUTIL), &attach_args(image, at), "could not open the download")?;
        Ok(Self(at.to_path_buf()))
    }

    fn detach(&self, force: bool) -> bool {
        install::run_tool(Path::new(HDIUTIL), &detach_args(&self.0, force), "could not close the download").is_ok()
    }
}

impl Drop for Mount {
    fn drop(&mut self) {
        if !self.detach(false) {
            self.detach(true);
        }
    }
}

#[cfg(test)]
mod tests;
