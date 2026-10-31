use std::io::{BufRead as _, BufReader};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream};
use std::process::{Child, ChildStderr, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result};
use futures::channel::mpsc::UnboundedSender;
use parking_lot::Mutex;

use super::Event;
use crate::panels::browser::chrome;

const START_WAIT: Duration = Duration::from_secs(20);
const PROBE_EVERY: Duration = Duration::from_millis(150);
const WATCH_EVERY: Duration = Duration::from_millis(400);
const QUICK: Duration = Duration::from_millis(60);
const REVERSE_RETRY: Duration = Duration::from_secs(30);

pub(crate) const REVERSE_PORTS: [u16; 5] = [9222, 9223, 9224, 9225, 9229];

const OPTIONS: &[&str] = &[
    "-N",
    "-T",
    "-o",
    "ExitOnForwardFailure=yes",
    "-o",
    "BatchMode=yes",
    "-o",
    "ConnectTimeout=10",
    "-o",
    "ServerAliveInterval=5",
    "-o",
    "ServerAliveCountMax=3",
    "-o",
    "ControlMaster=no",
    "-o",
    "ControlPath=none",
    "-o",
    "RemoteCommand=none",
    "-o",
    "LogLevel=ERROR",
];

pub(crate) fn forward_args(destination: &str, local: u16, remote: u16) -> Vec<String> {
    with_options(&["-L".into(), format!("127.0.0.1:{local}:localhost:{remote}")], destination)
}

pub(crate) fn reverse_args(destination: &str, (remote, browser): (u16, u16), probe: u16) -> Vec<String> {
    let forwards = [
        "-R".into(),
        format!("127.0.0.1:{remote}:127.0.0.1:{browser}"),
        "-L".into(),
        format!("127.0.0.1:{probe}:127.0.0.1:{remote}"),
    ];
    with_options(&forwards, destination)
}

fn with_options(forwards: &[String], destination: &str) -> Vec<String> {
    let options = OPTIONS.iter().map(|o| (*o).to_string());
    options.chain(forwards.iter().cloned()).chain(["--".to_string(), destination.to_string()]).collect()
}

static RUNNING: Mutex<Vec<(u64, Child)>> = Mutex::new(Vec::new());
static NEXT: AtomicU64 = AtomicU64::new(1);

fn spawn(args: &[String]) -> Result<(u64, Option<ChildStderr>)> {
    let mut child = Command::new("ssh")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("could not start ssh")?;
    let stderr = child.stderr.take();
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    RUNNING.lock().push((id, child));
    Ok((id, stderr))
}

pub(crate) fn stop(id: u64) {
    let child = {
        let mut running = RUNNING.lock();
        running.iter().position(|(i, _)| *i == id).map(|at| running.swap_remove(at).1)
    };
    if let Some(child) = child {
        end(child);
    }
}

fn end(mut child: Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn exited(id: u64) -> Option<bool> {
    let mut running = RUNNING.lock();
    let at = running.iter().position(|(i, _)| *i == id)?;
    let done = !matches!(running[at].1.try_wait(), Ok(None));
    if done {
        running.swap_remove(at);
    }
    Some(done)
}

pub(crate) fn quit() {
    *REVERSE.lock() = Reverse::Idle;
    let all = std::mem::take(&mut *RUNNING.lock());
    for (_, child) in all {
        end(child);
    }
}

pub(crate) fn free(port: u16) -> bool {
    let used = |ip: IpAddr| TcpStream::connect_timeout(&SocketAddr::new(ip, port), QUICK).is_ok();
    !used(Ipv4Addr::LOCALHOST.into())
        && !used(Ipv6Addr::LOCALHOST.into())
        && TcpListener::bind((Ipv4Addr::LOCALHOST, port)).is_ok()
}

pub(crate) fn any_free() -> Option<u16> {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).ok()?.local_addr().ok().map(|at| at.port())
}

fn listening(port: u16) -> bool {
    TcpStream::connect_timeout(&SocketAddr::from((Ipv4Addr::LOCALHOST, port)), QUICK).is_ok()
}

enum Waited {
    Ready,
    Exited,
    Stopped,
    Late,
}

enum Outcome {
    Stopped,
    Failed(String),
    Lost(String),
}

fn wait_ready(id: u64, probe: &dyn Fn() -> bool) -> Waited {
    let started = Instant::now();
    loop {
        match exited(id) {
            None => return Waited::Stopped,
            Some(true) => return Waited::Exited,
            Some(false) if probe() => return Waited::Ready,
            Some(false) => {}
        }
        if started.elapsed() > START_WAIT {
            stop(id);
            return Waited::Late;
        }
        thread::sleep(PROBE_EVERY);
    }
}

fn wait_exit(id: u64) -> bool {
    loop {
        match exited(id) {
            Some(false) => thread::sleep(WATCH_EVERY),
            Some(true) => return true,
            None => return false,
        }
    }
}

fn last_line(stderr: Option<ChildStderr>) -> String {
    let Some(stderr) = stderr else { return String::new() };
    let lines = BufReader::new(stderr).lines().map_while(Result::ok);
    lines.map(|line| line.trim().to_string()).filter(|line| !line.is_empty()).last().unwrap_or_default()
}

fn follow(id: u64, stderr: Option<ChildStderr>, probe: &dyn Fn() -> bool, on_ready: &dyn Fn()) -> Outcome {
    let reason = thread::spawn(move || last_line(stderr));
    let why = move || {
        let text = reason.join().unwrap_or_default();
        if text.is_empty() { STOPPED.to_string() } else { text }
    };
    match wait_ready(id, probe) {
        Waited::Stopped => Outcome::Stopped,
        Waited::Late => Outcome::Failed(format!("ssh did not connect within {} s", START_WAIT.as_secs())),
        Waited::Exited => Outcome::Failed(why()),
        Waited::Ready => {
            on_ready();
            if wait_exit(id) { Outcome::Lost(why()) } else { Outcome::Stopped }
        }
    }
}

const STOPPED: &str = "ssh stopped";

pub(crate) fn forward(destination: &str, (local, remote): (u16, u16), events: UnboundedSender<Event>) -> Result<u64> {
    let (id, stderr) = spawn(&forward_args(destination, local, remote))?;
    thread::spawn(move || {
        let ready = || {
            let _ = events.unbounded_send(Event::Ready(id));
        };
        if let Outcome::Failed(why) | Outcome::Lost(why) = follow(id, stderr, &|| listening(local), &ready) {
            let _ = events.unbounded_send(Event::Ended(id, why));
        }
    });
    Ok(id)
}

#[derive(Clone, PartialEq, Eq)]
struct Key {
    host: String,
    browser: u16,
}

enum Reverse {
    Idle,
    Starting(Key),
    Ready(Key, u16, u64),
    Failed(Key, Instant),
}

static REVERSE: Mutex<Reverse> = Mutex::new(Reverse::Idle);

pub(crate) fn reverse(host: &str, browser: u16, events: &UnboundedSender<Event>) -> Option<u16> {
    let key = Key { host: host.to_string(), browser };
    let mut state = REVERSE.lock();
    match &*state {
        Reverse::Ready(k, remote, _) if *k == key => return Some(*remote),
        Reverse::Starting(k) if *k == key => return None,
        Reverse::Failed(k, at) if *k == key && at.elapsed() < REVERSE_RETRY => return None,
        Reverse::Ready(_, _, process) => stop(*process),
        _ => {}
    }
    *state = Reverse::Starting(key.clone());
    let events = events.clone();
    thread::spawn(move || open_reverse(&key, &events));
    None
}

pub(crate) fn drop_reverse_unless(host: Option<&str>) {
    let mut state = REVERSE.lock();
    let (key, process) = match &*state {
        Reverse::Ready(key, _, process) => (key, Some(*process)),
        Reverse::Starting(key) | Reverse::Failed(key, _) => (key, None),
        Reverse::Idle => return,
    };
    if Some(key.host.as_str()) == host {
        return;
    }
    if let Some(process) = process {
        stop(process);
    }
    *state = Reverse::Idle;
}

fn settle(from: impl Fn(&Reverse) -> bool, to: Reverse) -> bool {
    let mut state = REVERSE.lock();
    let ours = from(&state);
    if ours {
        *state = to;
    }
    ours
}

fn open_reverse(key: &Key, events: &UnboundedSender<Event>) {
    let mut why = String::new();
    for remote in REVERSE_PORTS {
        match try_reverse(key, remote, events) {
            Some(failed) => why = failed,
            None => return,
        }
    }
    if settle(|s| matches!(s, Reverse::Starting(k) if k == key), Reverse::Failed(key.clone(), Instant::now())) {
        let _ = events.unbounded_send(Event::ReverseLost(why));
    }
}

fn try_reverse(key: &Key, remote: u16, events: &UnboundedSender<Event>) -> Option<String> {
    let Some(probe) = any_free() else { return Some("no free port on this Mac".into()) };
    let (id, stderr) = match spawn(&reverse_args(&key.host, (remote, key.browser), probe)) {
        Ok(started) => started,
        Err(e) => return Some(format!("{e:#}")),
    };
    let ready = || {
        let starting = |s: &Reverse| matches!(s, Reverse::Starting(k) if k == key);
        if settle(starting, Reverse::Ready(key.clone(), remote, id)) {
            let _ = events.unbounded_send(Event::ReverseReady);
        } else {
            stop(id);
        }
    };
    match follow(id, stderr, &|| chrome::version(probe).is_some(), &ready) {
        Outcome::Stopped => None,
        Outcome::Failed(why) => Some(why),
        Outcome::Lost(why) => {
            if settle(|s| matches!(s, Reverse::Ready(_, _, p) if *p == id), Reverse::Idle) {
                let _ = events.unbounded_send(Event::ReverseLost(why));
            }
            None
        }
    }
}

#[cfg(test)]
mod tests;
