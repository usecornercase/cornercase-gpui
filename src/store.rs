use std::time::{Duration, Instant};

use futures::StreamExt as _;
use futures::channel::mpsc;
use gpui::{AnyView, App, AppContext as _, AsyncApp, Context, Entity, Global, WeakEntity};
use serde_json::Value;

use crate::cli::{self, Queue, Stream};
use crate::model::Session;
use crate::panels::PanelKind;

pub(crate) struct Store {
    pub session: Option<Session>,
    pub view_id: Option<u64>,
    pub queue: Queue,
    pub app: Option<Value>,
    pub dock: Option<PanelKind>,
    pub sheet: Option<PanelKind>,
    pub dialog: Option<AnyView>,
    pub toasts: Vec<Toast>,
    next_toast: u64,
    app_stream: Option<Stream>,
}

#[derive(Clone)]
pub(crate) struct Toast {
    pub id: u64,
    pub text: String,
    pub kind: ToastKind,
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
            app_stream: None,
        });
        cx.set_global(GlobalStore(store.clone()));
        store
    }

    pub(crate) fn get(cx: &App) -> Entity<Store> {
        cx.global::<GlobalStore>().0.clone()
    }

    pub(crate) fn follow_app(&mut self, cx: &mut Context<Self>) {
        self.app_stream = follow(cx, args(&["status", "--app", "--follow"]), |this, line, cx| {
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                this.app = Some(value);
                cx.notify();
            }
        });
    }

    pub(crate) fn with_view(&self, mut args: Vec<String>) -> Vec<String> {
        if let Some(id) = self.view_id {
            args.push("--view".into());
            args.push(id.to_string());
        }
        args
    }

    pub(crate) fn act(&self, args: Vec<String>) {
        self.queue.cmd(self.with_view(args));
    }

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

pub(crate) fn args(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| (*s).to_string()).collect()
}

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

pub(crate) fn follow<T: 'static>(
    cx: &mut Context<T>,
    args: Vec<String>,
    on_line: impl Fn(&mut T, String, &mut Context<T>) + 'static,
) -> Option<Stream> {
    follow_until(cx, args, on_line, |_, _, _| {})
}

enum Line {
    Text(String),
    End(String),
}

pub(crate) fn follow_until<T: 'static>(
    cx: &mut Context<T>,
    args: impl AsRef<[String]>,
    on_line: impl Fn(&mut T, String, &mut Context<T>) + 'static,
    on_end: impl FnOnce(&mut T, String, &mut Context<T>) + 'static,
) -> Option<Stream> {
    let (tx, rx) = mpsc::unbounded::<Line>();
    let end = tx.clone();
    let stream = cli::stream(
        args.as_ref(),
        move |line| {
            let _ = tx.unbounded_send(Line::Text(line));
        },
        move |err| {
            let _ = end.unbounded_send(Line::End(err));
        },
    );
    match stream {
        Ok(stream) => {
            cx.spawn(async move |this, cx| deliver(this, rx, on_line, on_end, cx).await).detach();
            Some(stream)
        }
        Err(e) => {
            let text = e.to_string();
            cx.spawn(async move |this, cx| {
                let _ = this.update(cx, |this, cx| on_end(this, text, cx));
            })
            .detach();
            None
        }
    }
}

async fn deliver<T: 'static>(
    this: WeakEntity<T>,
    mut rx: mpsc::UnboundedReceiver<Line>,
    on_line: impl Fn(&mut T, String, &mut Context<T>),
    on_end: impl FnOnce(&mut T, String, &mut Context<T>),
    cx: &mut AsyncApp,
) {
    while let Some(line) = rx.next().await {
        match line {
            Line::Text(text) => {
                if this.update(cx, |this, cx| on_line(this, text, cx)).is_err() {
                    return;
                }
            }
            Line::End(err) => {
                let _ = this.update(cx, |this, cx| on_end(this, err, cx));
                return;
            }
        }
    }
}
