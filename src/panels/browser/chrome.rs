use std::ffi::OsString;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write as _};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};
use parking_lot::Mutex;

use crate::desktop;
use crate::platform::{self, Os};

const PREFERRED_PORT: u16 = 9222;
const ACTIVE_PORT: &str = "DevToolsActivePort";
const OWNER: &str = "cornercase-owner";
const PORT_FILE: &str = "cornercase-port";
const SNAP_BIN: &str = "/snap/bin";
const START_WAIT: Duration = Duration::from_secs(20);
const POLL: Duration = Duration::from_millis(100);
const LAST_WORDS: Duration = Duration::from_secs(1);
const LINE_MAX: u64 = 4096;

pub(crate) const NO_CHROME: &str = "No Chrome, Chromium, Edge or Brave was found";

const NAMES: &[&str] = &[
    "google-chrome-stable",
    "google-chrome",
    "chromium",
    "chromium-browser",
    "microsoft-edge-stable",
    "microsoft-edge",
    "brave-browser",
    "brave",
    "chrome",
    "chrome-headless-shell",
];

const MAC_APPS: &[&str] = &["Google Chrome", "Chromium", "Microsoft Edge", "Brave Browser", "Google Chrome Canary"];

const WINDOWS_APPS: &[&str] = &[
    "Google\\Chrome\\Application\\chrome.exe",
    "Microsoft\\Edge\\Application\\msedge.exe",
    "BraveSoftware\\Brave-Browser\\Application\\brave.exe",
    "Chromium\\Application\\chrome.exe",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Endpoint {
    pub(crate) port: u16,
    pub(crate) url: String,
}

#[derive(Default)]
struct Owned {
    child: Option<Child>,
    pid: Option<u32>,
}

static OWNED: Mutex<Owned> = Mutex::new(Owned { child: None, pid: None });

struct Chosen {
    exe: PathBuf,
    profile: PathBuf,
}

static CHOSEN: OnceLock<Chosen> = OnceLock::new();

#[derive(Default)]
struct Said {
    error: Option<String>,
    ended: bool,
}

pub(crate) fn find() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("CORNERCASE_GPUI_CHROME") {
        return Some(PathBuf::from(path)).filter(|p| p.is_file());
    }
    let on_path = || NAMES.iter().find_map(|name| desktop::on_path(name));
    candidates(&home()).into_iter().find(|p| p.is_file()).or_else(on_path)
}

fn chosen_in(cell: &OnceLock<Chosen>, look: impl FnOnce() -> Option<PathBuf>) -> Option<&Chosen> {
    if let Some(chosen) = cell.get() {
        return Some(chosen);
    }
    let exe = look()?;
    let profile = profile_for(Some(&exe));
    Some(cell.get_or_init(|| Chosen { exe, profile }))
}

fn home() -> PathBuf {
    desktop::home().unwrap_or_default()
}

pub(crate) fn candidates(home: &Path) -> Vec<PathBuf> {
    if cfg!(target_os = "macos") {
        let roots = [PathBuf::from("/Applications"), home.join("Applications")];
        return roots
            .iter()
            .flat_map(|root| MAC_APPS.iter().map(move |app| root.join(format!("{app}.app/Contents/MacOS/{app}"))))
            .collect();
    }
    if cfg!(windows) {
        let bases = ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"];
        let bases: Vec<PathBuf> = bases.iter().filter_map(std::env::var_os).map(PathBuf::from).collect();
        return bases.iter().flat_map(|base| WINDOWS_APPS.iter().map(move |app| base.join(app))).collect();
    }
    Vec::new()
}

pub(crate) fn profile() -> PathBuf {
    profile_in(&CHOSEN)
}

fn profile_in(cell: &OnceLock<Chosen>) -> PathBuf {
    cell.get().map_or_else(|| profile_for(None), |chosen| chosen.profile.clone())
}

fn profile_for(exe: Option<&Path>) -> PathBuf {
    if let Some(dir) = std::env::var_os("CORNERCASE_GPUI_BROWSER_PROFILE") {
        return PathBuf::from(dir);
    }
    match exe.and_then(snap_of) {
        Some(snap) => snap_profile(&home(), &snap),
        None => crate::prefs::app_dir().join("browser"),
    }
}

fn snap_profile(home: &Path, snap: &str) -> PathBuf {
    home.join("snap").join(snap).join("common/cornercase-gpui-browser")
}

fn snap_of(exe: &Path) -> Option<String> {
    let real = fs::canonicalize(exe).ok()?;
    let mut head = Vec::new();
    if let Ok(file) = fs::File::open(&real) {
        let _ = file.take(4096).read_to_end(&mut head);
    }
    let command = snap_command(exe, &real, &head)?;
    let link = fs::read_link(Path::new(SNAP_BIN).join(&command)).ok();
    snap_name(&command, link.as_deref())
}

fn snap_command(exe: &Path, real: &Path, head: &[u8]) -> Option<String> {
    if exe.starts_with(SNAP_BIN) || real.file_name().is_some_and(|name| name == "snap") {
        return exe.file_name()?.to_str().map(str::to_string);
    }
    let script = String::from_utf8_lossy(head.strip_prefix(b"#!")?);
    script.lines().find_map(runs_snap)
}

fn runs_snap(line: &str) -> Option<String> {
    let words: Vec<&str> = line.split_whitespace().map(|word| word.trim_matches(['"', '\''])).collect();
    let exec = words.iter().position(|word| *word == "exec");
    let program = exec.map_or(words.first(), |at| words.get(at + 1))?;
    let command = Path::new(program).strip_prefix(SNAP_BIN).ok()?.to_str()?;
    (!command.is_empty() && !command.contains('/')).then(|| command.to_string())
}

fn snap_name(command: &str, link: Option<&Path>) -> Option<String> {
    let alias = link.filter(|target| target.parent() == Some(Path::new(""))).and_then(Path::to_str);
    alias.unwrap_or(command).split('.').next().filter(|name| !name.is_empty()).map(str::to_string)
}

pub(crate) fn args(os: Os, profile: &Path, port: u16) -> Vec<OsString> {
    let mut user_data = OsString::from("--user-data-dir=");
    user_data.push(profile);
    let keyring = match os {
        Os::Mac => Some("--use-mock-keychain"),
        Os::Linux => Some("--password-store=basic"),
        Os::Windows => None,
    };
    let flags = [
        "--remote-debugging-address=127.0.0.1",
        "--no-first-run",
        "--no-default-browser-check",
        "--hide-crash-restore-bubble",
        "--disable-features=Translate,MediaRouter",
        "--force-color-profile=srgb",
        "--window-size=1280,800",
    ];
    let mut args = vec!["--headless".into(), user_data, format!("--remote-debugging-port={port}").into()];
    args.extend(flags.map(OsString::from));
    args.extend(keyring.map(OsString::from));
    args.push("--no-startup-window".into());
    args
}

pub(crate) fn active_port(text: &str) -> Option<Endpoint> {
    let mut lines = text.lines();
    let port: u16 = lines.next()?.trim().parse().ok().filter(|p| *p > 0)?;
    let path = lines.next()?.trim();
    path.starts_with('/').then(|| Endpoint { port, url: format!("ws://127.0.0.1:{port}{path}") })
}

pub(crate) fn attach_or_launch() -> Result<Endpoint> {
    let chosen = chosen_in(&CHOSEN, find);
    let profile = profile();
    fs::create_dir_all(&profile).with_context(|| format!("could not make {}", profile.display()))?;
    if let Some(endpoint) = running(&profile) {
        adopt_if_orphan(&profile);
        return Ok(endpoint);
    }
    launch(&chosen.context(NO_CHROME)?.exe, &profile)
}

fn known_port(profile: &Path) -> Option<u16> {
    let read = |name: &str| fs::read_to_string(profile.join(name)).ok();
    read(ACTIVE_PORT)
        .and_then(|text| active_port(&text).map(|e| e.port))
        .or_else(|| read(PORT_FILE).and_then(|text| text.trim().parse().ok()))
}

fn running(profile: &Path) -> Option<Endpoint> {
    if !browser_pid(profile).is_some_and(alive) {
        return None;
    }
    let endpoint = version(known_port(profile)?)?;
    let _ = fs::write(profile.join(PORT_FILE), endpoint.port.to_string());
    Some(endpoint)
}

pub(super) fn version(port: u16) -> Option<Endpoint> {
    let at = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&at, Duration::from_millis(400)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    stream.write_all(b"GET /json/version HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n").ok()?;
    browser_url(&read_response(&mut BufReader::new(stream))?, port)
}

fn read_response(reader: &mut impl BufRead) -> Option<String> {
    let mut length = 0usize;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).ok().filter(|n| *n > 0)?;
        if line.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.trim().eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse().ok()?;
        }
    }
    let mut body = vec![0u8; length.min(1 << 20)];
    reader.read_exact(&mut body).ok()?;
    String::from_utf8(body).ok()
}

pub(crate) fn browser_url(response: &str, port: u16) -> Option<Endpoint> {
    let body = response.split_once("\r\n\r\n").map_or(response, |(_, body)| body);
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let url = value["webSocketDebuggerUrl"].as_str()?;
    let path = &url[url.find("/devtools/")?..];
    Some(Endpoint { port, url: format!("ws://127.0.0.1:{port}{path}") })
}

fn launch(exe: &Path, profile: &Path) -> Result<Endpoint> {
    let _ = fs::remove_file(profile.join(ACTIVE_PORT));
    let port = if free(PREFERRED_PORT) { PREFERRED_PORT } else { 0 };
    let _ = fs::write(profile.join(PORT_FILE), port.to_string());
    let mut child = platform::command(exe)
        .args(args(Os::CURRENT, profile, port))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("could not start {}", exe.display()))?;
    let said = Arc::new(Mutex::new(Said::default()));
    if let Some(stderr) = child.stderr.take() {
        let said = said.clone();
        std::thread::spawn(move || keep_errors(stderr, &said));
    }
    let _ = fs::write(profile.join(OWNER), std::process::id().to_string());
    let pid = child.id();
    *OWNED.lock() = Owned { child: Some(child), pid: Some(pid) };
    wait_for_port(profile, &said)
}

fn keep_errors(stderr: impl Read, said: &Mutex<Said>) {
    let mut reader = BufReader::new(stderr);
    let mut line = Vec::new();
    while (&mut reader).take(LINE_MAX).read_until(b'\n', &mut line).is_ok_and(|read| read > 0) {
        if let Some(error) = error_in(&String::from_utf8_lossy(&line)) {
            said.lock().error = Some(error);
        }
        line.clear();
    }
    said.lock().ended = true;
}

fn error_in(line: &str) -> Option<String> {
    let (head, text) = line.strip_prefix('[')?.split_once("] ")?;
    (head.contains(":ERROR:") || head.contains(":FATAL:")).then(|| text.trim().chars().take(500).collect())
}

fn last_words(said: &Mutex<Said>) -> Option<String> {
    let started = Instant::now();
    while !said.lock().ended && started.elapsed() < LAST_WORDS {
        std::thread::sleep(POLL);
    }
    said.lock().error.clone()
}

fn free(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

fn wait_for_port(profile: &Path, said: &Mutex<Said>) -> Result<Endpoint> {
    let started = Instant::now();
    while started.elapsed() < START_WAIT {
        if let Some(endpoint) = running(profile) {
            return Ok(endpoint);
        }
        let ended = OWNED.lock().child.as_mut().and_then(|c| c.try_wait().ok().flatten());
        if let Some(status) = ended {
            match last_words(said) {
                Some(error) => bail!("the browser stopped as it started ({status}): {error}"),
                None => bail!("the browser stopped as it started ({status})"),
            }
        }
        std::thread::sleep(POLL);
    }
    bail!("the browser did not open its DevTools port within {} s", START_WAIT.as_secs())
}

fn adopt_if_orphan(profile: &Path) {
    let owner = fs::read_to_string(profile.join(OWNER)).ok().and_then(|text| text.trim().parse::<u32>().ok());
    if owner.is_some_and(alive) {
        return;
    }
    let Some(pid) = browser_pid(profile) else { return };
    let _ = fs::write(profile.join(OWNER), std::process::id().to_string());
    *OWNED.lock() = Owned { child: None, pid: Some(pid) };
}

pub(crate) fn lock_pid(target: &str) -> Option<u32> {
    target.rsplit_once('-').and_then(|(_, pid)| pid.parse().ok())
}

fn browser_pid(profile: &Path) -> Option<u32> {
    let target = fs::read_link(profile.join("SingletonLock")).ok()?;
    lock_pid(&target.to_string_lossy())
}

#[cfg(unix)]
fn alive(pid: u32) -> bool {
    libc::pid_t::try_from(pid).is_ok_and(|pid| unsafe { libc::kill(pid, 0) } == 0)
}

#[cfg(not(unix))]
fn alive(_pid: u32) -> bool {
    true
}

#[cfg(unix)]
fn stop(pid: u32) {
    if let Ok(pid) = libc::pid_t::try_from(pid) {
        unsafe { libc::kill(pid, libc::SIGTERM) };
    }
}

#[cfg(not(unix))]
fn stop(_pid: u32) {}

pub(crate) fn quit() {
    let owned = std::mem::take(&mut *OWNED.lock());
    match (owned.child, owned.pid) {
        (Some(child), Some(pid)) => end(child, pid),
        (None, Some(pid)) => stop(pid),
        _ => return,
    }
    let _ = fs::remove_file(profile().join(OWNER));
}

fn end(mut child: Child, pid: u32) {
    stop(pid);
    let started = Instant::now();
    while started.elapsed() < Duration::from_millis(1500) {
        if child.try_wait().ok().flatten().is_some() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests;
