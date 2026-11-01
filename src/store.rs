use std::time::{Duration, Instant};

use futures::StreamExt as _;
use futures::channel::{mpsc, oneshot};
use gpui::{AnyView, App, AppContext as _, AsyncApp, ClipboardItem, Context, Entity, Global, WeakEntity};
use serde_json::Value;

use crate::cli::{self, Queue, Stream, Target};
use crate::model::Session;
use crate::panels::PanelKind;
use crate::prefs::{self, Prefs};

mod jobs;

pub(crate) use jobs::Busy;
use jobs::Jobs;

const NO_FLOW: &str = "This workspace runs no flow";

pub(crate) struct Store {
    pub session: Option<Session>,
    pub view_id: Option<u64>,
    pub queue: Queue,
    pub app: Option<Value>,
    pub dock: Option<PanelKind>,
    pub sheet: Option<PanelKind>,
    pub dialog: Option<AnyView>,
    pub toasts: Vec<Toast>,
    pub switch_to: Option<Target>,
    pub browse: Option<(String, Option<u64>)>,
    pub prefs: Prefs,
    pub started_run_in: Option<u64>,
    pub jobs: Jobs,
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
    pub count: u32,
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

    fn repeats(&self, other: &Toast) -> bool {
        self.kind == other.kind && self.text == other.text && self.action == other.action && !self.expired()
    }
}

pub(crate) fn push_toast(toasts: &mut Vec<Toast>, mut toast: Toast) {
    if let Some(at) = toasts.iter().position(|t| t.repeats(&toast)) {
        toast.count += toasts.remove(at).count;
    }
    toasts.push(toast);
    if toasts.len() > 5 {
        toasts.remove(0);
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
            switch_to: None,
            browse: None,
            prefs: prefs::load(),
            started_run_in: None,
            jobs: Jobs::default(),
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

    pub(crate) fn act_busy(&mut self, busy: Busy, args: Vec<String>, cx: &mut Context<Self>) {
        let id = self.start_job(busy, cx);
        let reply = reply_of(&self.queue, self.with_view(args));
        cx.spawn(async move |this, cx| {
            let result = reply.await;
            let _ = this.update(cx, |s, cx| s.answered(id, result.ok(), cx));
        })
        .detach();
    }

    fn answered(&mut self, id: u64, answer: Option<anyhow::Result<String>>, cx: &mut Context<Self>) {
        self.end_job(id, cx);
        if let Some(Err(e)) = answer {
            self.toast(e.to_string(), ToastKind::Error, cx);
        }
    }

    pub(crate) fn focus(&self, kind: &str, id: u64) {
        self.act(args(&["focus", &format!("--{kind}"), &id.to_string()]));
    }

    pub(crate) fn toast(&mut self, text: impl Into<String>, kind: ToastKind, cx: &mut Context<Self>) {
        self.toast_with(text, kind, None, cx);
    }

    pub(crate) fn ask_agent(&mut self, workspace: u64, text: &str, label: &str, cx: &mut Context<Self>) {
        let Some((pane, agent)) = self.session.as_ref().and_then(|s| s.workspace_agent(workspace)) else {
            cx.write_to_clipboard(ClipboardItem::new_string(text.trim_end().to_string()));
            self.toast(format!("No agent in this workspace: copied {label}"), ToastKind::Info, cx);
            return;
        };
        self.cmd(args(&["send", "--pane", &pane.to_string(), "--", text]));
        self.focus("pane", pane);
        self.toast(format!("Sent {label} to {agent}"), ToastKind::Info, cx);
    }

    pub(crate) fn copy_elsewhere(&mut self, path: String, cx: &mut Context<Self>) {
        let text = format!("Copied {path}: it is on {}, not on this Mac", cli::host().unwrap_or_default());
        cx.write_to_clipboard(ClipboardItem::new_string(path));
        self.toast(text, ToastKind::Info, cx);
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
        push_toast(&mut self.toasts, Toast { id, text: text.into(), kind, action, at: Instant::now(), count: 1 });
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

    pub(crate) fn start_job(&mut self, busy: Busy, cx: &mut Context<Self>) -> u64 {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(jobs::SHOW_AFTER).await;
            let _ = this.update(cx, |_, cx| cx.notify());
        })
        .detach();
        cx.notify();
        self.jobs.start(busy)
    }

    pub(crate) fn end_job(&mut self, id: u64, cx: &mut Context<Self>) {
        self.jobs.end(id);
        cx.notify();
    }

    pub(crate) fn busy(&self, key: &str) -> bool {
        self.jobs.busy(key)
    }

    pub(crate) fn removing(&self, workspace: u64) -> bool {
        self.jobs.removing(workspace)
    }

    pub(crate) fn run_aside(&mut self, busy: Busy, args: Vec<String>, cx: &mut Context<Self>) {
        let id = self.start_job(busy, cx);
        run_job(cx, id, args, |this, result, cx| {
            if let Err(e) = result {
                this.toast(e.to_string(), ToastKind::Error, cx);
            }
        });
    }

    pub(crate) fn remove_worktree(&mut self, workspace: u64, name: &str, args: Vec<String>, cx: &mut Context<Self>) {
        let id = self.start_job(Busy::remove_worktree(workspace, name), cx);
        run_job(cx, id, args, move |this, result, cx| this.settle_removal(workspace, &result, cx));
    }

    pub(crate) fn settle_removal(&mut self, workspace: u64, result: &anyhow::Result<String>, cx: &mut Context<Self>) {
        match result {
            Ok(_) => self.jobs.removed(workspace),
            Err(e) => self.toast(e.to_string(), ToastKind::Error, cx),
        }
        cx.notify();
    }

    pub(crate) fn dismiss_toast(&mut self, id: u64, cx: &mut Context<Self>) {
        self.toasts.retain(|t| t.id != id);
        cx.notify();
    }

    pub(crate) fn toggle_dock(&mut self, kind: PanelKind, cx: &mut Context<Self>) {
        if !kind.shows(self.session.as_ref()) {
            self.toast(NO_FLOW, ToastKind::Info, cx);
            return;
        }
        self.dock = if self.dock == Some(kind) { None } else { Some(kind) };
        cx.notify();
    }

    pub(crate) fn shown_dock(&self) -> Option<PanelKind> {
        self.dock.filter(|kind| kind.shows(self.session.as_ref()))
    }

    pub(crate) fn show_run(&mut self, cx: &mut Context<Self>) {
        self.sheet = None;
        self.dock = Some(PanelKind::Flow);
        cx.notify();
    }

    pub(crate) fn toggle(&mut self, kind: PanelKind, cx: &mut Context<Self>) {
        if kind.docked() {
            self.toggle_dock(kind, cx);
        } else if self.sheet == Some(kind) {
            self.close_sheet(cx);
        } else {
            self.open_sheet(kind, cx);
        }
    }

    pub(crate) fn close_overlay(&mut self, cx: &mut Context<Self>) -> bool {
        let closed = self.dialog.take().is_some() || self.sheet.take().is_some();
        if closed {
            cx.notify();
        }
        closed
    }

    pub(crate) fn close_dock(&mut self, cx: &mut Context<Self>) -> bool {
        let closed = self.dock.take().is_some();
        if closed {
            cx.notify();
        }
        closed
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

    pub(crate) fn open_in_browser(&mut self, url: String, workspace: Option<u64>, cx: &mut Context<Self>) {
        self.browse = Some((url, workspace));
        self.open(PanelKind::Browser, cx);
    }

    pub(crate) fn set_pref(&mut self, key: &str, value: &str, cx: &mut Context<Self>) -> anyhow::Result<()> {
        let changed = self.prefs.with(key, value)?;
        prefs::save(&changed)?;
        self.prefs = changed;
        cx.notify();
        Ok(())
    }

    pub(crate) fn run_started(&mut self, workspace: u64, cx: &mut Context<Self>) {
        self.started_run_in = Some(workspace);
        cx.notify();
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

    pub(crate) fn connect(&mut self, target: Target, cx: &mut Context<Self>) {
        self.dialog = None;
        if target != cli::target() {
            self.switch_to = Some(target);
        }
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
    run_with(cx, None, move || cli::run(&args), then);
}

pub(crate) fn run_partial<T: 'static>(
    cx: &mut Context<T>,
    args: Vec<String>,
    then: impl FnOnce(&mut T, anyhow::Result<String>, &mut Context<T>) + 'static,
) {
    run_with(cx, None, move || cli::run_partial(&args), then);
}

pub(crate) fn run_busy<T: 'static>(
    cx: &mut Context<T>,
    busy: Busy,
    args: Vec<String>,
    then: impl FnOnce(&mut T, anyhow::Result<String>, &mut Context<T>) + 'static,
) {
    let id = Store::get(cx).update(cx, |s, cx| s.start_job(busy, cx));
    run_job(cx, id, args, then);
}

fn run_job<T: 'static>(
    cx: &mut Context<T>,
    id: u64,
    args: Vec<String>,
    then: impl FnOnce(&mut T, anyhow::Result<String>, &mut Context<T>) + 'static,
) {
    let job = (Store::get(cx), id);
    run_with(cx, Some(job), move || cli::run(&args), then);
}

fn run_with<T: 'static>(
    cx: &mut Context<T>,
    job: Option<(Entity<Store>, u64)>,
    work: impl FnOnce() -> anyhow::Result<String> + Send + 'static,
    then: impl FnOnce(&mut T, anyhow::Result<String>, &mut Context<T>) + 'static,
) {
    let task = cx.background_spawn(async move { work() });
    cx.spawn(async move |this, cx| {
        let result = task.await;
        if let Some((store, id)) = job {
            let _ = store.update(cx, |s, cx| s.end_job(id, cx));
        }
        let _ = this.update(cx, |this, cx| then(this, result, cx));
    })
    .detach();
}

fn reply_of(queue: &Queue, args: Vec<String>) -> oneshot::Receiver<anyhow::Result<String>> {
    let (tx, rx) = oneshot::channel();
    queue.cmd_then(args, move |result| {
        let _ = tx.send(result);
    });
    rx
}

pub(crate) fn queued(cx: &App, args: Vec<String>, then: impl FnOnce(anyhow::Result<String>, &mut App) + 'static) {
    let rx = reply_of(&Store::get(cx).read(cx).queue, args);
    answered_on(cx, None, rx, then);
}

pub(crate) fn queued_busy(
    cx: &mut App,
    busy: Busy,
    args: Vec<String>,
    then: impl FnOnce(anyhow::Result<String>, &mut App) + 'static,
) {
    let store = Store::get(cx);
    let id = store.update(cx, |s, cx| s.start_job(busy, cx));
    let rx = reply_of(&store.read(cx).queue, args);
    answered_on(cx, Some((store, id)), rx, then);
}

fn answered_on(
    cx: &App,
    job: Option<(Entity<Store>, u64)>,
    rx: oneshot::Receiver<anyhow::Result<String>>,
    then: impl FnOnce(anyhow::Result<String>, &mut App) + 'static,
) {
    cx.spawn(async move |cx| {
        let result = rx.await;
        if let Some((store, id)) = job {
            let _ = store.update(cx, |s, cx| s.end_job(id, cx));
        }
        let Ok(result) = result else { return };
        let _ = cx.update(|cx| then(result, cx));
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

#[cfg(test)]
mod tests;
