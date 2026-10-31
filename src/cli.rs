mod partial;
mod remote;
mod requests;
mod typist;

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;

use anyhow::{Context as _, Result, bail};
use parking_lot::Mutex;

pub(crate) use partial::run_partial;
pub(crate) use remote::{Remote, backoff, from_home};
use requests::{Answered, Requests};
use typist::{OnError, Typist};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum Target {
    #[default]
    Local,
    Remote(Remote),
}

impl Target {
    pub(crate) fn from_args(args: &[String]) -> Result<Self> {
        Ok(Remote::from_args(args)?.map_or(Self::Local, Self::Remote))
    }

    pub(crate) fn host(&self) -> Option<&str> {
        match self {
            Self::Local => None,
            Self::Remote(remote) => Some(remote.destination()),
        }
    }

    fn command<S: AsRef<str>>(&self, args: &[S]) -> Command {
        let mut cmd = Command::new(bin());
        if let Self::Remote(remote) = self {
            cmd.args(remote.flags());
        }
        cmd.args(args.iter().map(AsRef::as_ref));
        for name in CALLER {
            cmd.env_remove(name);
        }
        cmd
    }
}

const CALLER: [&str; 3] = ["CORNERCASE_PANE", "CORNERCASE_SERVER", "CORNERCASE_REMOTE"];

struct Current {
    target: Target,
    generation: u64,
}

static CURRENT: Mutex<Current> = Mutex::new(Current { target: Target::Local, generation: 0 });

pub(crate) fn target() -> Target {
    CURRENT.lock().target.clone()
}

pub(crate) fn host() -> Option<String> {
    CURRENT.lock().target.host().map(str::to_string)
}

pub(crate) fn connect(target: Target) -> Target {
    let mut current = CURRENT.lock();
    current.generation += 1;
    std::mem::replace(&mut current.target, target)
}

fn generation() -> u64 {
    CURRENT.lock().generation
}

pub(crate) fn bin() -> String {
    std::env::var("CORNERCASE_BIN").unwrap_or_else(|_| "cornercase".to_string())
}

pub(crate) fn command<S: AsRef<str>>(args: &[S]) -> Command {
    target().command(args)
}

pub(crate) fn run<S: AsRef<str>>(args: &[S]) -> Result<String> {
    run_with_stdin(args, None)
}

pub(crate) fn run_with_stdin<S: AsRef<str>>(args: &[S], stdin: Option<&str>) -> Result<String> {
    run_on(&target(), args, stdin.map(str::as_bytes))
}

fn run_on<S: AsRef<str>>(target: &Target, args: &[S], stdin: Option<&[u8]>) -> Result<String> {
    let mut cmd = target.command(args);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd.stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() });
    let mut child = cmd.spawn().with_context(|| format!("could not run `{}`", bin()))?;
    if let Some(text) = stdin
        && let Some(mut pipe) = child.stdin.take()
    {
        let _ = pipe.write_all(text);
    }
    let out = child.wait_with_output()?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let name = args.first().map(AsRef::as_ref).unwrap_or_default();
        bail!("cornercase {name}: {}", if err.is_empty() { "failed".to_string() } else { err });
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub(crate) fn quit() {
    kill_all();
    let target = target();
    if let Some(mut child) = clear_theme(&target)
        && target == Target::Local
    {
        let _ = child.wait();
    }
}

pub(crate) fn leave(target: &Target) {
    let Some(mut child) = clear_theme(target) else { return };
    if *target == Target::Local {
        let _ = child.wait();
    } else {
        thread::spawn(move || child.wait());
    }
}

fn clear_theme(target: &Target) -> Option<Child> {
    let mut cmd = target.command(&["theme", "--clear"]);
    cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    cmd.spawn().ok()
}

static CHILDREN: OnceLock<Mutex<Vec<(u64, Child)>>> = OnceLock::new();
static NEXT_CHILD: AtomicU64 = AtomicU64::new(1);

fn children() -> &'static Mutex<Vec<(u64, Child)>> {
    CHILDREN.get_or_init(|| Mutex::new(Vec::new()))
}

pub(crate) struct Stream {
    id: u64,
}

impl Drop for Stream {
    fn drop(&mut self) {
        let mut list = children().lock();
        if let Some(i) = list.iter().position(|(id, _)| *id == self.id) {
            let (_, mut child) = list.swap_remove(i);
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub(crate) fn kill_all() {
    let mut list = children().lock();
    for (_, child) in list.iter_mut() {
        let _ = child.kill();
        let _ = child.wait();
    }
    list.clear();
}

pub(crate) fn stream<S: AsRef<str>>(
    args: &[S],
    on_line: impl FnMut(String) + Send + 'static,
    on_end: impl FnOnce(String) + Send + 'static,
) -> Result<Stream> {
    let target = target();
    let mut cmd = target.command(args);
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().with_context(|| format!("could not run `{}`", bin()))?;
    let stdout = child.stdout.take().context("no stdout")?;
    let stderr = child.stderr.take().context("no stderr")?;
    let stream = keep(child);
    let err_reader = thread::spawn(move || read_all(stderr));
    thread::spawn(move || {
        read_lines(stdout, on_line);
        on_end(err_reader.join().unwrap_or_default());
    });
    Ok(stream)
}

fn keep(child: Child) -> Stream {
    let id = NEXT_CHILD.fetch_add(1, Ordering::Relaxed);
    children().lock().push((id, child));
    Stream { id }
}

struct Piped {
    stdin: ChildStdin,
    stdout: ChildStdout,
    stderr: ChildStderr,
    process: Stream,
}

fn piped(target: &Target, args: &[&str]) -> Option<Piped> {
    let mut cmd = target.command(args);
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().ok()?;
    let pipes = (child.stdin.take(), child.stdout.take(), child.stderr.take());
    let process = keep(child);
    let (Some(stdin), Some(stdout), Some(stderr)) = pipes else { return None };
    Some(Piped { stdin, stdout, stderr, process })
}

fn read_all(stderr: ChildStderr) -> String {
    let mut text = String::new();
    for line in BufReader::new(stderr).lines().map_while(Result::ok) {
        text.push_str(&line);
        text.push('\n');
    }
    text
}

fn read_lines(stdout: ChildStdout, mut on_line: impl FnMut(String)) {
    let mut reader = BufReader::with_capacity(1 << 20, stdout);
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => on_line(std::mem::take(&mut line)),
        }
    }
}

enum Job {
    Input { pane: u64, event: serde_json::Value },
    Scroll { pane: u64, rows: i64 },
    Cmd(Cmd),
    Reconnect,
}

struct Cmd {
    args: Vec<String>,
    coalesce: Option<&'static str>,
    reply: Option<Reply>,
    stdin: Option<Vec<u8>>,
}

type Reply = Box<dyn FnOnce(Result<String>) + Send>;

type Typed = Vec<(u64, serde_json::Value)>;

const MAX_INPUT_BATCH: usize = 256;

#[derive(Clone)]
pub(crate) struct Queue {
    tx: mpsc::Sender<Job>,
}

impl Queue {
    pub(crate) fn new(on_error: impl Fn(String) + Send + Sync + 'static) -> Self {
        let (tx, rx) = mpsc::channel::<Job>();
        let mut worker = {
            let current = CURRENT.lock();
            Worker {
                target: current.target.clone(),
                generation: current.generation,
                on_error: Arc::new(on_error),
                typist: None,
                requests: None,
            }
        };
        thread::spawn(move || worker.work(&rx));
        Self { tx }
    }

    pub(crate) fn input(&self, pane: u64, event: serde_json::Value) {
        let _ = self.tx.send(Job::Input { pane, event });
    }

    pub(crate) fn scroll(&self, pane: u64, rows: i64) {
        let _ = self.tx.send(Job::Scroll { pane, rows });
    }

    pub(crate) fn cmd(&self, args: Vec<String>) {
        let _ = self.tx.send(Job::Cmd(Cmd { args, coalesce: None, reply: None, stdin: None }));
    }

    pub(crate) fn cmd_with_stdin(&self, args: Vec<String>, stdin: Vec<u8>) {
        let _ = self.tx.send(Job::Cmd(Cmd { args, coalesce: None, reply: None, stdin: Some(stdin) }));
    }

    pub(crate) fn coalesced(&self, key: &'static str, args: Vec<String>) {
        let _ = self.tx.send(Job::Cmd(Cmd { args, coalesce: Some(key), reply: None, stdin: None }));
    }

    pub(crate) fn cmd_then(&self, args: Vec<String>, reply: impl FnOnce(Result<String>) + Send + 'static) {
        let _ = self.tx.send(Job::Cmd(Cmd { args, coalesce: None, reply: Some(Box::new(reply)), stdin: None }));
    }

    pub(crate) fn reconnect(&self) {
        let _ = self.tx.send(Job::Reconnect);
    }
}

struct Worker {
    target: Target,
    generation: u64,
    on_error: OnError,
    typist: Option<Typist>,
    requests: Option<Requests>,
}

impl Worker {
    fn work(&mut self, rx: &mpsc::Receiver<Job>) {
        let mut pending: Option<Job> = None;
        loop {
            let Some(job) = pending.take().or_else(|| rx.recv().ok()) else { return };
            if generation() != self.generation {
                return;
            }
            pending = match job {
                Job::Input { pane, event } => self.send_input(rx, (pane, event)),
                Job::Scroll { pane, rows } => self.scroll(rx, pane, rows),
                Job::Cmd(cmd) => self.run_cmd(rx, cmd),
                Job::Reconnect => self.reconnect(),
            };
        }
    }

    fn reconnect(&mut self) -> Option<Job> {
        self.typist = None;
        self.requests = None;
        None
    }

    fn send_input(&mut self, rx: &mpsc::Receiver<Job>, first: (u64, serde_json::Value)) -> Option<Job> {
        let (events, pending) = gather_input(rx, first);
        if !self.type_through_typist(&events) {
            self.type_one_shot(&events);
        }
        pending
    }

    fn type_through_typist(&mut self, events: &Typed) -> bool {
        let lines: Vec<String> = events.iter().map(|(pane, event)| typist::line(*pane, event)).collect();
        if self.typist.as_mut().is_some_and(|typist| typist.send(&lines)) {
            return true;
        }
        self.typist = Typist::start(&self.target, &self.on_error);
        let sent = self.typist.as_mut().is_some_and(|typist| typist.send(&lines));
        if !sent {
            self.typist = None;
        }
        sent
    }

    fn type_one_shot(&self, events: &Typed) {
        for run in events.chunk_by(|a, b| a.0 == b.0) {
            let args = ["input".to_string(), "--pane".to_string(), run[0].0.to_string(), "-".to_string()];
            let lines = run.iter().map(|(_, event)| event.to_string()).collect::<Vec<_>>().join("\n") + "\n";
            if let Err(e) = run_on(&self.target, &args, Some(lines.as_bytes())) {
                (self.on_error)(e.to_string());
            }
        }
    }

    fn settle(&mut self) {
        if self.typist.as_mut().is_some_and(|typist| !typist.settle()) {
            self.typist = None;
        }
    }

    fn scroll(&mut self, rx: &mpsc::Receiver<Job>, pane: u64, rows: i64) -> Option<Job> {
        let (rows, pending) = gather_scroll(rx, pane, rows);
        if rows != 0 {
            self.run_cmd(rx, Cmd { args: scroll_args(pane, rows), coalesce: None, reply: None, stdin: None });
        }
        pending
    }

    fn run_cmd(&mut self, rx: &mpsc::Receiver<Job>, cmd: Cmd) -> Option<Job> {
        let (cmd, pending) = match cmd.coalesce {
            Some(key) => coalesce(rx, key, cmd),
            None => (cmd, None),
        };
        self.settle();
        let result = self.run(&cmd.args, cmd.stdin.as_deref());
        match (cmd.reply, result) {
            (Some(reply), result) => reply(result),
            (None, Err(e)) => (self.on_error)(e.to_string()),
            (None, Ok(_)) => {}
        }
        pending
    }

    fn run(&mut self, args: &[String], stdin: Option<&[u8]>) -> Result<String> {
        let name = args.first().map_or("", String::as_str);
        match requests::line(args, stdin).and_then(|line| self.ask(&line)) {
            Some(Ok(answer)) => requests::answer(name, &answer),
            Some(Err(why)) => Err(requests::lost(name, &why)),
            None => run_on(&self.target, args, stdin),
        }
    }

    fn ask(&mut self, line: &str) -> Option<Answered> {
        let answer = self.requests.as_mut().and_then(|requests| requests.ask(line)).or_else(|| {
            self.requests = Requests::start(&self.target);
            self.requests.as_mut().and_then(|requests| requests.ask(line))
        });
        if !matches!(answer, Some(Ok(_))) {
            self.requests = None;
        }
        answer
    }
}

fn gather_input(rx: &mpsc::Receiver<Job>, first: (u64, serde_json::Value)) -> (Typed, Option<Job>) {
    let mut events = vec![first];
    while events.len() < MAX_INPUT_BATCH {
        match rx.try_recv() {
            Ok(Job::Input { pane, event }) => events.push((pane, event)),
            Ok(other) => return (events, Some(other)),
            Err(_) => break,
        }
    }
    (events, None)
}

fn gather_scroll(rx: &mpsc::Receiver<Job>, pane: u64, mut rows: i64) -> (i64, Option<Job>) {
    loop {
        match rx.try_recv() {
            Ok(Job::Scroll { pane: next, rows: more }) if next == pane => rows += more,
            Ok(other) => return (rows, Some(other)),
            Err(_) => return (rows, None),
        }
    }
}

fn scroll_args(pane: u64, rows: i64) -> Vec<String> {
    let dir = if rows > 0 { "--up" } else { "--down" };
    vec!["scroll".into(), "--pane".into(), pane.to_string(), dir.into(), rows.unsigned_abs().to_string()]
}

fn coalesce(rx: &mpsc::Receiver<Job>, key: &'static str, mut cmd: Cmd) -> (Cmd, Option<Job>) {
    loop {
        match rx.try_recv() {
            Ok(Job::Cmd(next)) if next.coalesce == Some(key) => cmd = next,
            Ok(other) => return (cmd, Some(other)),
            Err(_) => return (cmd, None),
        }
    }
}

pub(crate) fn reason(error: &anyhow::Error) -> String {
    let text = error.to_string();
    let text = match text.split_once(": ") {
        Some((head, rest)) if head.starts_with("cornercase ") => rest.to_string(),
        _ => text,
    };
    let text = text.strip_prefix("Error: ").or_else(|| text.strip_prefix("error: ")).unwrap_or(&text);
    text.trim().to_string()
}

#[cfg(test)]
mod tests;
