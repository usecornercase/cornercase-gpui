use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStderr, ChildStdout, Command, Stdio};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;

use anyhow::{Context as _, Result, bail};
use parking_lot::Mutex;

pub(crate) fn bin() -> String {
    std::env::var("CORNERCASE_BIN").unwrap_or_else(|_| "cornercase".to_string())
}

pub(crate) fn command<S: AsRef<str>>(args: &[S]) -> Command {
    let mut cmd = Command::new(bin());
    cmd.args(args.iter().map(AsRef::as_ref));
    cmd.env_remove("CORNERCASE_PANE");
    cmd.env_remove("CORNERCASE_SERVER");
    cmd
}

pub(crate) fn run<S: AsRef<str>>(args: &[S]) -> Result<String> {
    run_with_stdin(args, None)
}

pub(crate) fn run_with_stdin<S: AsRef<str>>(args: &[S], stdin: Option<&str>) -> Result<String> {
    let mut cmd = command(args);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd.stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() });
    let mut child = cmd.spawn().with_context(|| format!("could not run `{}`", bin()))?;
    if let Some(text) = stdin
        && let Some(mut pipe) = child.stdin.take()
    {
        pipe.write_all(text.as_bytes())?;
    }
    let out = child.wait_with_output()?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let name = args.first().map(AsRef::as_ref).unwrap_or_default();
        bail!("cornercase {name}: {}", if err.is_empty() { "failed".to_string() } else { err });
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
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
    let mut cmd = command(args);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());
    let mut child = cmd.spawn().with_context(|| format!("could not run `{}`", bin()))?;
    let stdout = child.stdout.take().context("no stdout")?;
    let stderr = child.stderr.take().context("no stderr")?;
    let id = NEXT_CHILD.fetch_add(1, Ordering::Relaxed);
    children().lock().push((id, child));

    let err_reader = thread::spawn(move || read_all(stderr));
    thread::spawn(move || {
        read_lines(stdout, on_line);
        on_end(err_reader.join().unwrap_or_default());
    });
    Ok(Stream { id })
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
    Cmd(Cmd),
}

struct Cmd {
    args: Vec<String>,
    coalesce: Option<&'static str>,
    reply: Option<Reply>,
}

type Reply = Box<dyn FnOnce(Result<String>) + Send>;

const MAX_INPUT_BATCH: usize = 256;

#[derive(Clone)]
pub(crate) struct Queue {
    tx: mpsc::Sender<Job>,
}

impl Queue {
    pub(crate) fn new(on_error: impl Fn(String) + Send + 'static) -> Self {
        let (tx, rx) = mpsc::channel::<Job>();
        thread::spawn(move || worker(&rx, &on_error));
        Self { tx }
    }

    pub(crate) fn input(&self, pane: u64, event: serde_json::Value) {
        let _ = self.tx.send(Job::Input { pane, event });
    }

    pub(crate) fn cmd(&self, args: Vec<String>) {
        let _ = self.tx.send(Job::Cmd(Cmd { args, coalesce: None, reply: None }));
    }

    pub(crate) fn coalesced(&self, key: &'static str, args: Vec<String>) {
        let _ = self.tx.send(Job::Cmd(Cmd { args, coalesce: Some(key), reply: None }));
    }

    pub(crate) fn cmd_then(&self, args: Vec<String>, reply: impl FnOnce(Result<String>) + Send + 'static) {
        let _ = self.tx.send(Job::Cmd(Cmd { args, coalesce: None, reply: Some(Box::new(reply)) }));
    }
}

fn worker(rx: &mpsc::Receiver<Job>, on_error: &dyn Fn(String)) {
    let mut pending: Option<Job> = None;
    loop {
        let Some(job) = pending.take().or_else(|| rx.recv().ok()) else { return };
        pending = match job {
            Job::Input { pane, event } => send_input(rx, pane, event, on_error),
            Job::Cmd(cmd) => run_cmd(rx, cmd, on_error),
        };
    }
}

fn send_input(rx: &mpsc::Receiver<Job>, pane: u64, first: serde_json::Value, on_error: &dyn Fn(String)) -> Option<Job> {
    let (events, pending) = gather_input(rx, pane, first);
    let mut args = vec!["input".to_string(), "--pane".to_string(), pane.to_string()];
    args.extend(events.iter().map(serde_json::Value::to_string));
    if let Err(e) = run(&args) {
        on_error(e.to_string());
    }
    pending
}

fn gather_input(
    rx: &mpsc::Receiver<Job>,
    pane: u64,
    first: serde_json::Value,
) -> (Vec<serde_json::Value>, Option<Job>) {
    let mut events = vec![first];
    while events.len() < MAX_INPUT_BATCH {
        match rx.try_recv() {
            Ok(Job::Input { pane: p, event }) if p == pane => events.push(event),
            Ok(other) => return (events, Some(other)),
            Err(_) => break,
        }
    }
    (events, None)
}

fn run_cmd(rx: &mpsc::Receiver<Job>, cmd: Cmd, on_error: &dyn Fn(String)) -> Option<Job> {
    let (cmd, pending) = match cmd.coalesce {
        Some(key) => coalesce(rx, key, cmd),
        None => (cmd, None),
    };
    let result = run(&cmd.args);
    match (cmd.reply, result) {
        (Some(reply), result) => reply(result),
        (None, Err(e)) => on_error(e.to_string()),
        (None, Ok(_)) => {}
    }
    pending
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
