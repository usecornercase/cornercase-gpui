//! State every part of the window shares: the session, our view, the app state that is not part
//! of the session (`cornercase status --app --follow`), which panel, sheet and dialog are open,
//! and the toasts. Panels read it and act through it; the root view owns the connection.

use std::time::{Duration, Instant};

use gpui::{AnyView, App, AppContext as _, Context, Entity, Global};
use serde_json::Value;

use crate::cli::{self, Queue, Stream};
use crate::model::Session;
use crate::panels::PanelKind;

pub(crate) struct Store {
    pub session: Option<Session>,
    pub view_id: Option<u64>,
    pub queue: Queue,
    /// What `cornercase status --app` prints: todos, queue, away, update, `keep_awake`, tour…
    pub app: Option<Value>,
    /// The panel docked on the right of the terminal, if any.
    pub dock: Option<PanelKind>,
    /// The sheet over the window (issues, usage, settings…), if any.
    pub sheet: Option<PanelKind>,
    /// A small dialog over everything (a form, a confirmation).
    pub dialog: Option<AnyView>,
    pub toasts: Vec<Toast>,
    next_toast: u64,
    _app_stream: Option<Stream>,
}

#[derive(Clone)]
pub(crate) struct Toast {
    pub id: u64,
    pub text: String,
    pub kind: ToastKind,
    /// A button: its label and the command it runs (`go TOKEN` for an agent's notice).
    pub action: Option<(String, Vec<String>)>,
    pub at: Instant,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToastKind {
    Info,
    Error,
    NeedsYou,
    Done,
}

impl Toast {
    pub(crate) fn expired(&self) -> bool {
        let ttl = match self.kind {
            ToastKind::Info => 4,
            ToastKind::Error => 8,
            ToastKind::Done | ToastKind::NeedsYou => 15,
        };
        self.at.elapsed() > Duration::from_secs(ttl)
    }
}

struct GlobalStore(Entity<Store>);
impl Global for GlobalStore {}

impl Store {
    pub(crate) fn init(queue: Queue, cx: &mut App) -> Entity<Store> {
        let store = cx.new(|_| Store {
            session: None,
            view_id: None,
            queue,
            app: None,
            dock: None,
            sheet: None,
            dialog: None,
            toasts: Vec::new(),
            next_toast: 1,
            _app_stream: None,
        });
        cx.set_global(GlobalStore(store.clone()));
        store
    }

    /// The store, from anywhere.
    pub(crate) fn get(cx: &App) -> Entity<Store> {
        cx.global::<GlobalStore>().0.clone()
    }

    /// Starts following `cornercase status --app --follow` (again after a reconnect).
    pub(crate) fn follow_app(&mut self, cx: &mut Context<Self>) {
        self._app_stream = follow(cx, args(&["status", "--app", "--follow"]), |this, line, cx| {
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                this.app = Some(value);
                cx.notify();
            }
        });
    }

    /// Adds `--view ID` to a command that takes one, so it acts in our view, not the window.
    pub(crate) fn with_view(&self, mut args: Vec<String>) -> Vec<String> {
        if let Some(id) = self.view_id {
            args.push("--view".into());
            args.push(id.to_string());
        }
        args
    }

    /// Runs a command in our view through the ordered queue; failures become toasts.
    pub(crate) fn act(&self, args: Vec<String>) {
        self.queue.cmd(self.with_view(args));
    }

    /// Runs a command that takes no view through the ordered queue; failures become toasts.
    pub(crate) fn cmd(&self, args: Vec<String>) {
        self.queue.cmd(args);
    }

    pub(crate) fn focus(&self, kind: &str, id: u64) {
        self.act(args(&["focus", &format!("--{kind}"), &id.to_string()]));
    }

    pub(crate) fn toast(&mut self, text: impl Into<String>, kind: ToastKind, cx: &mut Context<Self>) {
        self.toast_with(text, kind, None, cx);
    }

    pub(crate) fn toast_with(
        &mut self,
        text: impl Into<String>,
        kind: ToastKind,
        action: Option<(String, Vec<String>)>,
        cx: &mut Context<Self>,
    ) {
        let id = self.next_toast;
        self.next_toast += 1;
        self.toasts.push(Toast { id, text: text.into(), kind, action, at: Instant::now() });
        if self.toasts.len() > 5 {
            self.toasts.remove(0);
        }
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(16)).await;
            let _ = this.update(cx, |this, cx| {
                this.toasts.retain(|t| !t.expired());
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn dismiss_toast(&mut self, id: u64, cx: &mut Context<Self>) {
        self.toasts.retain(|t| t.id != id);
        cx.notify();
    }

    pub(crate) fn toggle_dock(&mut self, kind: PanelKind, cx: &mut Context<Self>) {
        self.dock = if self.dock == Some(kind) { None } else { Some(kind) };
        cx.notify();
    }

    pub(crate) fn open_sheet(&mut self, kind: PanelKind, cx: &mut Context<Self>) {
        self.sheet = Some(kind);
        cx.notify();
    }

    /// Opens a panel where it belongs: docked or as a sheet.
    pub(crate) fn open(&mut self, kind: PanelKind, cx: &mut Context<Self>) {
        if kind.docked() {
            self.dock = Some(kind);
            cx.notify();
        } else {
            self.open_sheet(kind, cx);
        }
    }

    pub(crate) fn close_sheet(&mut self, cx: &mut Context<Self>) {
        self.sheet = None;
        cx.notify();
    }

    pub(crate) fn show_dialog(&mut self, view: AnyView, cx: &mut Context<Self>) {
        self.dialog = Some(view);
        cx.notify();
    }

    pub(crate) fn close_dialog(&mut self, cx: &mut Context<Self>) {
        self.dialog = None;
        cx.notify();
    }
}

/// `["a", "b"]` as the owned arguments the helpers take.
pub(crate) fn args(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| (*s).to_string()).collect()
}

/// Runs a cornercase command off the main thread, then hands its standard output (or the error)
/// to `then` on the entity. For commands whose answer the UI needs; fire-and-forget actions go
/// through `Store::act` or `Store::cmd`, which keep their order.
pub(crate) fn run<T: 'static>(
    cx: &mut Context<T>,
    args: Vec<String>,
    then: impl FnOnce(&mut T, anyhow::Result<String>, &mut Context<T>) + 'static,
) {
    let task = cx.background_spawn(async move { cli::run(&args) });
    cx.spawn(async move |this, cx| {
        let result = task.await;
        let _ = this.update(cx, |this, cx| then(this, result, cx));
    })
    .detach();
}

/// Like `run`, with the output read as JSON (commands given `--json`).
pub(crate) fn run_json<T: 'static>(
    cx: &mut Context<T>,
    args: Vec<String>,
    then: impl FnOnce(&mut T, anyhow::Result<Value>, &mut Context<T>) + 'static,
) {
    run(cx, args, move |this, result, cx| {
        let value = result.and_then(|out| serde_json::from_str::<Value>(&out).map_err(anyhow::Error::from));
        then(this, value, cx);
    });
}

/// Follows a command that prints one line per change (`changes --follow`, `events --json`…):
/// `on_line` runs on the entity for each line. Keep the returned stream: dropping it stops the
/// command. When it ends, `on_line` gets nothing more; call `follow` again to resume.
pub(crate) fn follow<T: 'static>(
    cx: &mut Context<T>,
    args: Vec<String>,
    on_line: impl Fn(&mut T, String, &mut Context<T>) + 'static,
) -> Option<Stream> {
    follow_until(cx, args, on_line, |_, _, _| {})
}

/// `follow`, with `on_end` called once the command stops, with what it wrote on stderr.
pub(crate) fn follow_until<T: 'static>(
    cx: &mut Context<T>,
    args: Vec<String>,
    on_line: impl Fn(&mut T, String, &mut Context<T>) + 'static,
    on_end: impl FnOnce(&mut T, String, &mut Context<T>) + 'static,
) -> Option<Stream> {
    use futures::StreamExt as _;
    enum Line {
        Text(String),
        End(String),
    }
    let (tx, mut rx) = futures::channel::mpsc::unbounded::<Line>();
    let end = tx.clone();
    let stream = cli::stream(
        &args,
        move |line| {
            let _ = tx.unbounded_send(Line::Text(line));
        },
        move |err| {
            let _ = end.unbounded_send(Line::End(err));
        },
    );
    let stream = match stream {
        Ok(stream) => stream,
        Err(e) => {
            let text = e.to_string();
            cx.spawn(async move |this, cx| {
                let _ = this.update(cx, |this, cx| on_end(this, text, cx));
            })
            .detach();
            return None;
        }
    };
    cx.spawn(async move |this, cx| {
        let mut on_end = Some(on_end);
        while let Some(line) = rx.next().await {
            let alive = match line {
                Line::Text(text) => this.update(cx, |this, cx| on_line(this, text, cx)).is_ok(),
                Line::End(err) => {
                    if let Some(on_end) = on_end.take() {
                        let _ = this.update(cx, |this, cx| on_end(this, err, cx));
                    }
                    false
                }
            };
            if !alive {
                break;
            }
        }
    })
    .detach();
    Some(stream)
}
