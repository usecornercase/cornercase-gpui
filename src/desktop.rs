use std::ffi::OsString;
use std::fs::{DirBuilder, File, OpenOptions};
use std::io::{self, Read as _, Seek as _};
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, anyhow, bail};
use gpui::App;

use crate::platform::{self, Os};

const SETTLE: Duration = Duration::from_secs(8);
const ANSWER: Duration = Duration::from_secs(30);
const POLL: Duration = Duration::from_millis(20);
const BUNDLES: &[&str] = &["app", "xcodeproj", "xcworkspace", "framework", "bundle", "playground"];
const FILE_MANAGER1: &str = "org.freedesktop.FileManager1";
const FILE_MANAGER1_PATH: &str = "/org/freedesktop/FileManager1";
const NO_OPENER: &str = "nothing on this computer opens files: install xdg-utils, which gives xdg-open";
const SCRATCH_TRIES: usize = 100;

pub(crate) const fn machine(os: Os) -> &'static str {
    match os {
        Os::Mac => "Mac",
        Os::Linux => "computer",
        Os::Windows => "PC",
    }
}

pub(crate) const fn here() -> &'static str {
    machine(Os::CURRENT)
}

pub(crate) const fn file_manager(os: Os) -> &'static str {
    match os {
        Os::Mac => "Finder",
        Os::Linux => "file manager",
        Os::Windows => "File Explorer",
    }
}

pub(crate) fn show_label(os: Os, dir: bool) -> String {
    match (os, dir) {
        (Os::Mac, false) => "Reveal in Finder".to_string(),
        (Os::Linux, false) => "Show in file manager".to_string(),
        (Os::Windows, false) => "Show in File Explorer".to_string(),
        (_, true) => format!("Open in {}", file_manager(os)),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Board {
    Clipboard,
    Primary,
}

pub(crate) const fn has_primary(os: Os) -> bool {
    matches!(os, Os::Linux)
}

pub(crate) const fn selection_board(os: Os) -> Board {
    if has_primary(os) { Board::Primary } else { Board::Clipboard }
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub(crate) fn write_primary(text: &str, cx: &App) {
    cx.write_to_primary(gpui::ClipboardItem::new_string(text.to_string()));
}

#[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
pub(crate) fn write_primary(_text: &str, _cx: &App) {}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub(crate) fn read_primary(cx: &App) -> Option<String> {
    cx.read_from_primary().and_then(|item| item.text()).filter(|text| !text.is_empty())
}

#[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
pub(crate) fn read_primary(_cx: &App) -> Option<String> {
    None
}

#[cfg(unix)]
pub(crate) fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
pub(crate) fn executable(path: &Path) -> bool {
    path.is_file()
}

pub(crate) fn find_in(dirs: &[PathBuf], name: &str) -> Option<PathBuf> {
    dirs.iter().filter(|dir| dir.is_absolute()).map(|dir| dir.join(name)).find(|path| executable(path))
}

pub(crate) fn path_dirs() -> Vec<PathBuf> {
    std::env::var_os("PATH").map(|path| std::env::split_paths(&path).collect()).unwrap_or_default()
}

pub(crate) fn on_path(name: &str) -> Option<PathBuf> {
    find_in(&path_dirs(), name)
}

pub(crate) fn data_dirs(home: Option<&Path>, data_home: Option<OsString>, shared: Option<OsString>) -> Vec<PathBuf> {
    let absolute = |dir: &PathBuf| dir.is_absolute();
    let own = data_home.map(PathBuf::from).filter(absolute).or_else(|| home.map(|h| h.join(".local/share")));
    let shared: Vec<PathBuf> = match shared.filter(|s| !s.is_empty()) {
        Some(dirs) => std::env::split_paths(&dirs).filter(absolute).collect(),
        None => vec![PathBuf::from("/usr/local/share"), PathBuf::from("/usr/share")],
    };
    own.into_iter().chain(shared).collect()
}

pub(crate) fn xdg_data_dirs() -> Vec<PathBuf> {
    data_dirs(home().as_deref(), std::env::var_os("XDG_DATA_HOME"), std::env::var_os("XDG_DATA_DIRS"))
}

pub(crate) fn home() -> Option<PathBuf> {
    std::env::home_dir().filter(|home| home.is_absolute())
}

pub(crate) fn home_text() -> String {
    home().map(|home| lossy(&home)).unwrap_or_default()
}

pub(crate) fn lossy(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn opener(os: Os, find: &dyn Fn(&str) -> Option<PathBuf>) -> Option<Vec<String>> {
    match os {
        Os::Mac => Some(vec!["open".to_string()]),
        Os::Linux => find("xdg-open")
            .map(|xdg| vec![lossy(&xdg)])
            .or_else(|| find("gio").map(|gio| vec![lossy(&gio), "open".to_string()])),
        Os::Windows => None,
    }
}

pub(crate) fn opens(os: Os, find: &dyn Fn(&str) -> Option<PathBuf>) -> bool {
    opener(os, find).is_some()
}

pub(crate) fn reveals(os: Os, find: &dyn Fn(&str) -> Option<PathBuf>) -> bool {
    opens(os, find) || ["gdbus", "dbus-send"].iter().any(|tool| find(tool).is_some())
}

pub(crate) fn open_argv(os: Os, path: &Path, find: &dyn Fn(&str) -> Option<PathBuf>) -> Option<Vec<String>> {
    let mut argv = opener(os, find)?;
    argv.push(lossy(path));
    Some(argv)
}

pub(crate) fn reveal_argv(
    os: Os,
    path: &Path,
    dir: bool,
    find: &dyn Fn(&str) -> Option<PathBuf>,
) -> Option<Vec<String>> {
    match os {
        Os::Mac => Some(finder_argv(path, dir)),
        Os::Linux => file_manager_call(path, dir, find),
        Os::Windows => None,
    }
}

fn finder_argv(path: &Path, dir: bool) -> Vec<String> {
    let bundle = dir && path.extension().and_then(|e| e.to_str()).is_some_and(|e| BUNDLES.contains(&e));
    if dir && !bundle { vec!["open".into(), lossy(path)] } else { vec!["open".into(), "-R".into(), lossy(path)] }
}

fn file_uri(path: &Path) -> Option<String> {
    let uri = url::Url::from_file_path(path).ok()?;
    Some(uri.as_str().replace('\'', "%27").replace(',', "%2C").replace('\\', "%5C"))
}

fn file_manager_call(path: &Path, dir: bool, find: &dyn Fn(&str) -> Option<PathBuf>) -> Option<Vec<String>> {
    let method = format!("{FILE_MANAGER1}.{}", if dir { "ShowFolders" } else { "ShowItems" });
    let uri = file_uri(path)?;
    if let Some(gdbus) = find("gdbus") {
        let head = ["call", "--session", "--dest", FILE_MANAGER1, "--object-path", FILE_MANAGER1_PATH, "--method"];
        let mut argv = vec![lossy(&gdbus)];
        argv.extend(head.map(String::from));
        argv.extend([method, format!("['{uri}']"), "''".to_string()]);
        return Some(argv);
    }
    let send = find("dbus-send")?;
    let head = ["--session", "--print-reply", "--type=method_call"];
    let mut argv = vec![lossy(&send)];
    argv.extend(head.map(String::from));
    argv.extend([format!("--dest={FILE_MANAGER1}"), FILE_MANAGER1_PATH.to_string(), method]);
    argv.extend([format!("array:string:{uri}"), "string:".to_string()]);
    Some(argv)
}

fn folder_of(path: &Path, dir: bool) -> &Path {
    if dir { path } else { path.parent().unwrap_or(path) }
}

pub(crate) fn open(path: &Path) -> Result<()> {
    launch(path).with_context(|| format!("could not open {}", path.display()))
}

fn launch(path: &Path) -> Result<()> {
    let argv = open_argv(Os::CURRENT, path, &on_path).ok_or_else(|| anyhow!(NO_OPENER))?;
    run(&argv)
}

pub(crate) fn reveal(path: &Path, dir: bool) -> Result<()> {
    let os = Os::CURRENT;
    let shown = reveal_argv(os, path, dir, &on_path).map(|argv| run_within(&argv, reveal_wait(os)));
    let result = match (os, shown) {
        (_, Some(Ok(()))) => Ok(()),
        (Os::Mac, Some(Err(e))) => Err(e),
        _ => launch(folder_of(path, dir)),
    };
    result.with_context(|| format!("could not show {} in the {}", path.display(), file_manager(os)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Late {
    Launched,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Wait {
    limit: Duration,
    late: Late,
}

const LAUNCH: Wait = Wait { limit: SETTLE, late: Late::Launched };

fn reveal_wait(os: Os) -> Wait {
    match os {
        Os::Mac | Os::Windows => LAUNCH,
        Os::Linux => Wait { limit: ANSWER, late: Late::Failed },
    }
}

pub(crate) fn run(argv: &[String]) -> Result<()> {
    run_within(argv, LAUNCH)
}

fn run_within(argv: &[String], wait: Wait) -> Result<()> {
    let (program, rest) = argv.split_first().context("nothing to run")?;
    let name = name_of(program);
    let log = scratch_file().context("could not make a temporary file")?;
    let mut said = log.try_clone().context("could not read a temporary file")?;
    let mut child = platform::command(program)
        .args(rest)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(log)
        .spawn()
        .with_context(|| format!("could not start {name}"))?;
    let started = Instant::now();
    while started.elapsed() < wait.limit {
        match child.try_wait()? {
            Some(status) if status.success() => return Ok(()),
            Some(status) => bail!(failure(&name, &status.to_string(), &read(&mut said))),
            None => std::thread::sleep(POLL),
        }
    }
    overdue(child, &name, wait)
}

fn overdue(mut child: Child, name: &str, wait: Wait) -> Result<()> {
    if wait.late == Late::Launched {
        std::thread::spawn(move || child.wait());
        return Ok(());
    }
    let _ = child.kill();
    let _ = child.wait();
    bail!("{name} did not answer within {} s", wait.limit.as_secs())
}

fn name_of(program: &str) -> String {
    Path::new(program).file_name().map_or_else(|| program.to_string(), |name| name.to_string_lossy().into_owned())
}

pub(crate) struct Scratch(PathBuf);

impl Scratch {
    pub(crate) fn folder(stem: &str) -> io::Result<Self> {
        fresh(stem, private_folder).map(|(path, ())| Self(path))
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(unix)]
fn private_folder(path: &Path) -> io::Result<()> {
    DirBuilder::new().mode(0o700).create(path)
}

#[cfg(not(unix))]
fn private_folder(path: &Path) -> io::Result<()> {
    DirBuilder::new().create(path)
}

fn fresh<T>(stem: &str, mut make: impl FnMut(&Path) -> io::Result<T>) -> io::Result<(PathBuf, T)> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    for _ in 0..SCRATCH_TRIES {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("cornercase-gpui-{stem}-{}-{n}", std::process::id()));
        match make(&path) {
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            result => return result.map(|made| (path, made)),
        }
    }
    Err(io::ErrorKind::AlreadyExists.into())
}

fn scratch_file() -> io::Result<File> {
    let open = |path: &Path| {
        let mut file = OpenOptions::new();
        file.read(true).write(true).create_new(true);
        #[cfg(unix)]
        file.mode(0o600);
        file.open(path)
    };
    let (path, file) = fresh("said", open)?;
    let _ = std::fs::remove_file(&path);
    Ok(file)
}

fn read(file: &mut File) -> String {
    let mut bytes = Vec::new();
    let _ = file.rewind();
    let _ = file.take(64 * 1024).read_to_end(&mut bytes);
    String::from_utf8_lossy(&bytes).into_owned()
}

pub(crate) fn failure(name: &str, status: &str, said: &str) -> String {
    let line = said.lines().map(str::trim).rfind(|line| !line.is_empty());
    match line {
        Some(line) => {
            let line = line.strip_prefix(&format!("{name}: ")).unwrap_or(line);
            format!("{name}: {}", line.chars().take(300).collect::<String>())
        }
        None => format!("{name} ended with {status}"),
    }
}

#[cfg(test)]
mod tests;
