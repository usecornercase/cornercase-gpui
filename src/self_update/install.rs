use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context as _, Result, bail};

use super::bundle;
use super::fetch::{self, ARCHIVE_LIMIT};
use super::release::{self, Asset, Latest};
use crate::platform::{self, Os, probe::lines_within};

const VERSION_WAIT: Duration = Duration::from_secs(5);
const OLD_COPIES: u32 = 100;
pub(super) const PROGRAM: &str = "cornercase-gpui";
const STAGE_PREFIX: &str = ".cornercase-gpui-update-";
const STALE_STAGE: Duration = Duration::from_hours(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Why {
    Bundle,
    Folder(PathBuf),
    Unplaced,
    NoDownload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Way {
    Replace { asset: Asset },
    Download { url: String, why: Why },
    Command(String),
}

pub(crate) fn way(
    released: bool,
    machine: (Os, &str),
    exe: Option<&Path>,
    latest: &Latest,
    writable: impl Fn(&Path) -> bool,
) -> Way {
    if !released {
        return Way::Command(release::source_command(&latest.version));
    }
    let (os, arch) = machine;
    let Some(asset) = release::asset_name(os, arch).and_then(|name| latest.asset(name)) else {
        return Way::Download { url: latest.page.clone(), why: Why::NoDownload };
    };
    let download = |why| Way::Download { url: asset.url.clone(), why };
    let Some(home) = exe.and_then(|exe| home(os, exe)) else {
        return download(if os == Os::Mac { Why::Bundle } else { Why::Unplaced });
    };
    let Some(folder) = home.parent() else { return download(Why::Unplaced) };
    if writable(folder) { Way::Replace { asset: asset.clone() } } else { download(Why::Folder(folder.to_path_buf())) }
}

pub(crate) fn home(os: Os, exe: &Path) -> Option<PathBuf> {
    if os == Os::Mac { bundle::of(exe) } else { Some(exe.to_path_buf()) }
}

pub(crate) fn writable(folder: &Path) -> bool {
    let probe = folder.join(format!(".{PROGRAM}-write-{}", std::process::id()));
    let made = fs::File::create(&probe).is_ok();
    let _ = fs::remove_file(&probe);
    made
}

pub(super) fn program_name(os: Os) -> String {
    if os == Os::Windows { format!("{PROGRAM}.exe") } else { PROGRAM.to_string() }
}

pub(super) struct Stage(PathBuf);

impl Stage {
    pub(super) fn beside(home: &Path) -> Result<Self> {
        let folder = home.parent().context("the program has no folder")?;
        let path = folder.join(format!("{STAGE_PREFIX}{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).with_context(|| format!("could not make {}", path.display()))?;
        Ok(Self(path))
    }

    pub(super) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(crate) fn install(latest: &Latest, asset: &Asset, exe: &Path, os: Os, https_only: bool) -> Result<String> {
    let download = download_checked(latest, asset, https_only)?;
    if os == Os::Mac {
        let app = bundle::of(exe).context("this copy is not inside cornercase.app")?;
        bundle::install(&download, &app, &latest.version)?;
    } else {
        let stage = Stage::beside(exe)?;
        let new = staged_binary(os, &download, stage.path())?;
        check_binary(&new, &latest.version)?;
        swap(os, &new, exe)?;
    }
    Ok(latest.version.clone())
}

fn download_checked(latest: &Latest, asset: &Asset, https_only: bool) -> Result<Vec<u8>> {
    let sums_name = release::sums_name(&asset.name);
    let sums = latest.asset(&sums_name).with_context(|| format!("the release has no {sums_name}"))?;
    let sums = fetch::download(&sums.url, fetch::NOTES_LIMIT, https_only, Some(fetch::NOTES_WAIT))?;
    let archive = fetch::download(&asset.url, ARCHIVE_LIMIT, https_only, None)?;
    if !release::matches(&archive, &String::from_utf8_lossy(&sums)) {
        bail!("the download does not match its checksum, so it was not installed");
    }
    Ok(archive)
}

fn staged_binary(os: Os, bytes: &[u8], stage: &Path) -> Result<PathBuf> {
    let archive = stage.join("download");
    fs::write(&archive, bytes).context("could not save the download")?;
    let into = stage.join("unpacked");
    fs::create_dir(&into).context("could not make a folder to unpack in")?;
    unpack(os, &archive, &into)?;
    let new = find_binary(&into, &program_name(os)).context("the download holds no cornercase-gpui program")?;
    make_runnable(&new)?;
    Ok(new)
}

pub(crate) fn unpack_command(os: Os, windows: Option<&Path>, archive: &Path, into: &Path) -> (PathBuf, Vec<OsString>) {
    let (program, flags) = if os == Os::Windows {
        let system = windows.map_or_else(|| PathBuf::from(r"C:\Windows"), Path::to_path_buf);
        (system.join("System32").join("tar.exe"), "-xf")
    } else {
        (PathBuf::from("tar"), "-xzf")
    };
    (program, vec![flags.into(), archive.into(), "-C".into(), into.into()])
}

fn unpack(os: Os, archive: &Path, into: &Path) -> Result<()> {
    let (program, args) = unpack_command(os, crate::desktop::windows_folder().as_deref(), archive, into);
    run_tool(&program, &args, "tar could not unpack the download")
}

pub(super) fn run_tool(program: &Path, args: &[OsString], failed: &str) -> Result<()> {
    let name = program.file_name().unwrap_or(program.as_os_str()).to_string_lossy();
    let out = platform::command(program).args(args).output().with_context(|| format!("could not run {name}"))?;
    if out.status.success() {
        return Ok(());
    }
    let said = String::from_utf8_lossy(&out.stderr);
    bail!("{failed}: {}", said.trim().lines().last().unwrap_or("it failed"))
}

pub(crate) fn find_binary(dir: &Path, name: &str) -> Option<PathBuf> {
    let direct = dir.join(name);
    if direct.is_file() {
        return Some(direct);
    }
    fs::read_dir(dir).ok()?.flatten().map(|entry| entry.path().join(name)).find(|path| path.is_file())
}

#[cfg(unix)]
fn make_runnable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).context("could not make the download runnable")
}

#[cfg(not(unix))]
#[expect(clippy::unnecessary_wraps, reason = "the same signature as the Unix one")]
fn make_runnable(_: &Path) -> Result<()> {
    Ok(())
}

pub(crate) fn verified(lines: &[String], version: &str) -> Result<()> {
    let said = lines.first().map(String::as_str).unwrap_or_default();
    match release::printed_version(said) {
        Some(printed) if printed == version => Ok(()),
        _ => bail!(
            "the downloaded program says “{}” instead of {PROGRAM} {version}, so it was not installed",
            said.trim()
        ),
    }
}

fn version_of(path: &Path) -> Result<Vec<String>, String> {
    let mut cmd = platform::command(path);
    cmd.arg("--version");
    lines_within(&mut cmd, VERSION_WAIT, |lines| !lines.is_empty())
}

pub(super) fn check_binary(new: &Path, version: &str) -> Result<()> {
    let lines = version_of(new).map_err(|why| anyhow::anyhow!("the downloaded program does not run here: {why}"))?;
    verified(&lines, version)
}

pub(crate) fn installed_version(exe: &Path) -> Option<String> {
    let lines = version_of(exe).ok()?;
    release::printed_version(lines.first()?).map(str::to_string)
}

pub(crate) fn old_copy(exe: &Path, n: u32) -> PathBuf {
    let stem = exe.file_stem().unwrap_or_default().to_string_lossy();
    let number = if n == 0 { String::new() } else { format!(".{n}") };
    let extension = exe.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    exe.with_file_name(format!("{stem}.old{number}{extension}"))
}

pub(crate) fn is_old_copy(file_name: &OsStr, exe: &Path) -> bool {
    (0..OLD_COPIES).any(|n| old_copy(exe, n).file_name() == Some(file_name))
}

pub(crate) fn free_old_copy(exe: &Path, free: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    (0..OLD_COPIES).map(|n| old_copy(exe, n)).find(|path| free(path))
}

fn free_after_removal(path: &Path) -> bool {
    !path.exists() || fs::remove_file(path).is_ok()
}

pub(crate) fn is_stage(file_name: &OsStr) -> bool {
    file_name.to_string_lossy().starts_with(STAGE_PREFIX)
}

fn stale(path: &Path) -> bool {
    let age = fs::metadata(path).and_then(|meta| meta.modified()).ok().and_then(|time| time.elapsed().ok());
    age.is_some_and(|age| age >= STALE_STAGE)
}

pub(crate) fn sweep(os: Os, exe: &Path) {
    let home = home(os, exe).unwrap_or_else(|| exe.to_path_buf());
    let Some(entries) = home.parent().and_then(|folder| fs::read_dir(folder).ok()) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if is_old_copy(&name, exe) {
            let _ = fs::remove_file(entry.path());
        } else if bundle::is_old(&name) || (is_stage(&name) && stale(&entry.path())) {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

pub(crate) fn swap(os: Os, new: &Path, exe: &Path) -> Result<()> {
    if os != Os::Windows {
        return fs::rename(new, exe).with_context(|| format!("could not replace {}", exe.display()));
    }
    let aside =
        free_old_copy(exe, free_after_removal).context("too many old copies of the program are still running")?;
    replace(new, exe, &aside)
}

pub(super) fn replace(new: &Path, old: &Path, aside: &Path) -> Result<()> {
    fs::rename(old, aside).with_context(|| format!("could not set {} aside", old.display()))?;
    let Err(placing) = fs::rename(new, old) else { return Ok(()) };
    match fs::rename(aside, old) {
        Ok(()) => Err(placing).context("could not put the new version in place; the old one stays"),
        Err(back) => bail!(
            "could not put the new version in place ({placing}) nor bring the old one back ({back}); it is at {}",
            aside.display()
        ),
    }
}

#[cfg(test)]
mod tests;
