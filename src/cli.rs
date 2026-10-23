//! Everything that talks to cornercase: one-shot commands, long-running streams and an ordered
//! queue for input. The GUI never speaks the socket protocol itself; it drives the `cornercase`
//! command line, which is the documented contract for programs that draw cornercase.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::OnceLock;
use std::thread;

use anyhow::{Context as _, Result, bail};
use parking_lot::Mutex;

/// The `cornercase` binary, overridable with `CORNERCASE_BIN`.
pub(crate) fn bin() -> String {
    std::env::var("CORNERCASE_BIN").unwrap_or_else(|_| "cornercase".to_string())
}

/// A command with the pane variables removed: launched from inside a cornercase pane, they would
/// make every command default to that pane instead of what our view shows.
pub(crate) fn command<S: AsRef<str>>(args: &[S]) -> Command {
    let mut cmd = Command::new(bin());
    cmd.args(args.iter().map(AsRef::as_ref));
    cmd.env_remove("CORNERCASE_PANE");
    cmd.env_remove("CORNERCASE_SERVER");
    cmd
}

/// Runs a command to the end and returns its standard output.
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

/// A long-running command (`view`, `surface`). Dropping it stops the process.
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

/// Stops every stream still running; called when the app quits.
pub(crate) fn kill_all() {
    let mut list = children().lock();
    for (_, child) in list.iter_mut() {
        let _ = child.kill();
        let _ = child.wait();
    }
    list.clear();
}

/// Starts a command that prints one JSON line per change. `on_line` runs on a reader thread for
/// each line, and `on_end` once the command's output ends, with what it wrote to stderr.
pub(crate) fn stream<S: AsRef<str>>(
    args: &[S],
    mut on_line: impl FnMut(String) + Send + 'static,
    on_end: impl FnOnce(String) + Send + 'static,
) -> Result<Stream> {
    let mut cmd = command(args);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());
    let mut child = cmd.spawn().with_context(|| format!("could not run `{}`", bin()))?;
    let stdout = child.stdout.take().context("no stdout")?;
    let stderr = child.stderr.take().context("no stderr")?;
    let id = NEXT_CHILD.fetch_add(1, Ordering::Relaxed);
    children().lock().push((id, child));

    let err_reader = thread::spawn(move || {
        let mut text = String::new();
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            text.push_str(&line);
            text.push('\n');
        }
        text
    });
    thread::spawn(move || {
        let mut reader = BufReader::with_capacity(1 << 20, stdout);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => on_line(std::mem::take(&mut line)),
            }
        }
        on_end(err_reader.join().unwrap_or_default());
    });
    Ok(Stream { id })
}

/// Work for the ordered queue: requests on different connections are not ordered, so input and
/// everything that must follow it goes through one thread, one command after the other.
pub(crate) enum Job {
    /// One input event (`cornercase input`) for a pane; consecutive ones are sent together.
    Input { pane: u64, event: serde_json::Value },
    /// Any other command. With a `coalesce` key, a run of jobs with the same key only sends the
    /// last one (a drag's selection, a divider being moved, the view's size).
    Cmd { args: Vec<String>, coalesce: Option<&'static str>, reply: Option<Reply> },
}

pub(crate) type Reply = Box<dyn FnOnce(Result<String>) + Send>;

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
        let _ = self.tx.send(Job::Cmd { args, coalesce: None, reply: None });
    }

    pub(crate) fn coalesced(&self, key: &'static str, args: Vec<String>) {
        let _ = self.tx.send(Job::Cmd { args, coalesce: Some(key), reply: None });
    }

    pub(crate) fn cmd_then(&self, args: Vec<String>, reply: impl FnOnce(Result<String>) + Send + 'static) {
        let _ = self.tx.send(Job::Cmd { args, coalesce: None, reply: Some(Box::new(reply)) });
    }
}

fn worker(rx: &mpsc::Receiver<Job>, on_error: &dyn Fn(String)) {
    let mut pending: Option<Job> = None;
    loop {
        let job = match pending.take() {
            Some(job) => job,
            None => match rx.recv() {
                Ok(job) => job,
                Err(_) => return,
            },
        };
        match job {
            Job::Input { pane, event } => {
                let mut events = vec![event];
                while events.len() < 256 {
                    match rx.try_recv() {
                        Ok(Job::Input { pane: p, event }) if p == pane => events.push(event),
                        Ok(other) => {
                            pending = Some(other);
                            break;
                        }
                        Err(_) => break,
                    }
                }
                let mut args = vec!["input".to_string(), "--pane".to_string(), pane.to_string()];
                args.extend(events.iter().map(serde_json::Value::to_string));
                if let Err(e) = run(&args) {
                    on_error(e.to_string());
                }
            }
            Job::Cmd { mut args, coalesce, mut reply } => {
                if let Some(key) = coalesce {
                    loop {
                        match rx.try_recv() {
                            Ok(Job::Cmd { args: a, coalesce: Some(k), reply: r }) if k == key => {
                                args = a;
                                reply = r;
                            }
                            Ok(other) => {
                                pending = Some(other);
                                break;
                            }
                            Err(_) => break,
                        }
                    }
                }
                let result = run(&args);
                match reply {
                    Some(reply) => reply(result),
                    None => {
                        if let Err(e) = result {
                            on_error(e.to_string());
                        }
                    }
                }
            }
        }
    }
}
