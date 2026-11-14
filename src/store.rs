use std::fmt::Display;
use std::sync::Arc;

use futures::StreamExt as _;
use futures::channel::{mpsc, oneshot};
use gpui::{AnyView, App, AppContext as _, AsyncApp, ClipboardItem, Context, Entity, FocusHandle, Global, WeakEntity};
use serde_json::Value;

use crate::cli::{self, Queue, Stream, Target};
use crate::desktop;
use crate::model::accounts::pick::{AccountSettings, AgentChoices};
use crate::model::accounts::{AccountsInfo, ListMoves, Selection, Switch, switching_key};
use crate::model::machines::{self, Machine};
use crate::model::{MachinePlace, Owner, Session};
use crate::notify::Posted;
use crate::notify::update::Updating;
use crate::panels::PanelKind;
use crate::panels::files::open::{self, Found};
use crate::panels::plugins::Offers;
use crate::panels::settings::FoundSetting;
use crate::prefs::{self, Prefs};

mod browse;
mod jobs;
mod machine_read;
mod machines_there;
mod run_on;
mod toasts;
mod tunnels;
mod update_there;

use jobs::Jobs;
pub(crate) use jobs::{Busy, setup_key};
use machine_read::ThereState;
pub(crate) use machine_read::{AgentReach, MachineRead, OlderNote, Reach, TabThere, release_clause};
pub(crate) use machines_there::{MachineAct, NoticeThere, answer_words, failed_there, part_sent_note, stale_note};
pub(crate) use run_on::{CloneThere, StartHere, StartThere};
pub(crate) use toasts::{Toast, ToastKind};
pub(crate) use tunnels::{
    Asked, Control, Held, MachineLink, ReverseState, Tail, TunnelRow, Whose, address_there, copied_words, heading,
    none_note, others_note, remote_url, section_tail,
};
pub(crate) use update_there::{
    AfterUpdate, UpdateOffer, UpdateThere, run_update, update_job, update_key, update_question,
};

const NO_FLOW: &str = "This workspace runs no flow";

fn editor_command(path: &str, line: impl Display) -> String {
    format!("${{VISUAL:-${{EDITOR:-vi}}}} +{line} {}", shell_quote(path))
}

fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

pub(crate) struct Store {
    pub session: Option<Session>,
    pub view_id: Option<u64>,
    pub queue: Queue,
    pub app: Option<Value>,
    pub machines: Vec<Machine>,
    pub machine_place: Option<MachinePlace>,
    placing: run_on::Placing,
    tunnels: tunnels::Tunnels,
    there: ThereState,
    pub dock: Option<PanelKind>,
    pub sheet: Option<PanelKind>,
    pub peeking: bool,
    pub dialog: Option<AnyView>,
    dialog_behind: Option<(AnyView, FocusHandle)>,
    pub toasts: Vec<Toast>,
    pub posted: Posted,
    pub switch_to: Option<Target>,
    pub asks: Asks,
    pub browse: Option<(String, Owner)>,
    pub landing: Option<FoundSetting>,
    pub prefs: Prefs,
    pub started_run_in: Option<u64>,
    pub project_sheet: Option<(PanelKind, u64)>,
    pub run_file: Option<String>,
    pub openers: Arc<Vec<Found>>,
    pub jobs: Jobs,
    pub plugins: Offers,
    pub keys_to_terminal: bool,
    pub file_full_screen: bool,
    pub accounts: AccountsInfo,
    pub selection: Selection,
    pub account_settings: AccountSettings,
    pub accounts_changes: u64,
    accounts_asked: u64,
    list_moves: ListMoves,
    next_toast: u64,
    app_stream: Option<Stream>,
}

#[derive(Default)]
pub(crate) struct Asks {
    pub refocus: Option<FocusHandle>,
    pub open_update: Option<Updating>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ToastGo {
    Run(Vec<String>),
    Machine(NoticeThere),
    Update(Updating),
    Browse(String),
    Tunnelled(String, u16),
    Job(Busy, Vec<String>),
    Open(PanelKind),
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
            machines: Vec::new(),
            machine_place: None,
            placing: run_on::Placing::default(),
            tunnels: tunnels::Tunnels::default(),
            there: ThereState::default(),
            dock: None,
            sheet: None,
            peeking: false,
            dialog: None,
            dialog_behind: None,
            toasts: Vec::new(),
            posted: Posted::default(),
            switch_to: None,
            asks: Asks::default(),
            browse: None,
            landing: None,
            prefs: prefs::load(),
            started_run_in: None,
            project_sheet: None,
            run_file: None,
            openers: Arc::default(),
            jobs: Jobs::default(),
            plugins: Offers::default(),
            keys_to_terminal: false,
            file_full_screen: false,
            accounts: AccountsInfo::default(),
            selection: Selection::default(),
            account_settings: AccountSettings::default(),
            accounts_changes: 0,
            accounts_asked: 0,
            list_moves: ListMoves::default(),
            next_toast: 1,
            app_stream: None,
        });
        Self::publish(store, cx)
    }

    fn publish(store: Entity<Store>, cx: &mut App) -> Entity<Store> {
        cx.set_global(GlobalStore(store.clone()));
        store.update(cx, |_, cx| detect_openers(cx));
        store
    }

    pub(crate) fn get(cx: &App) -> Entity<Store> {
        cx.global::<GlobalStore>().0.clone()
    }

    pub(crate) fn follow_app(&mut self, cx: &mut Context<Self>) {
        self.list_moves = ListMoves::default();
        self.app_stream = follow(cx, args(&["status", "--app", "--follow"]), |this, line, cx| {
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                this.take_app(value, cx);
            }
        });
    }

    fn take_app(&mut self, value: Value, cx: &mut Context<Self>) {
        self.machines = machines::in_app(&value);
        self.selection = Selection::of(&value);
        if self.list_moves.app_line(&value) {
            self.list_accounts(cx);
        }
        self.app = Some(value);
        self.follow_machine_tunnels(cx);
        self.ask_picked_home(cx);
        cx.notify();
    }

    pub(crate) fn take_machines(&mut self, machines: Vec<Machine>, cx: &mut Context<Self>) {
        self.machines = machines;
        self.follow_machine_tunnels(cx);
        self.ask_picked_home(cx);
        cx.notify();
    }

    fn list_accounts(&mut self, cx: &mut Context<Self>) {
        self.accounts_asked += 1;
        let asked = self.accounts_asked;
        run(cx, args(&["account", "list", "--json"]), move |this, result, cx| {
            if asked == this.accounts_asked {
                this.accounts_listed(result, cx);
            }
        });
    }

    fn accounts_listed(&mut self, result: anyhow::Result<String>, cx: &mut Context<Self>) {
        let listed = result.and_then(|out| Ok(serde_json::from_str::<AccountsInfo>(&out)?));
        match listed {
            Ok(accounts) if accounts == self.accounts => {}
            Ok(accounts) => {
                self.accounts = accounts;
                self.accounts_changes += 1;
                cx.notify();
            }
            Err(e) => self.toast(format!("Could not list the accounts: {}", cli::reason(&e)), ToastKind::Error, cx),
        }
    }

    pub(crate) fn account_choices(&self) -> Vec<AgentChoices> {
        self.account_settings.offered(&self.accounts)
    }

    pub(crate) fn take_account_settings(&mut self, settings: AccountSettings, cx: &mut Context<Self>) {
        if self.account_settings != settings {
            self.account_settings = settings;
            cx.notify();
        }
    }

    pub(crate) fn follow_accounts(&mut self, session: &Session, cx: &mut Context<Self>) {
        if self.list_moves.view_line(session) {
            self.list_accounts(cx);
        }
    }

    pub(crate) fn switch_account(&mut self, switch: Switch, cx: &mut Context<Self>) {
        let id = self.start_job(Busy::new(switching_key(switch.pane), switch.doing()), cx);
        let args = switch.args();
        run_job(cx, id, args, move |this, result, cx| this.account_switched(&switch, result, cx));
    }

    fn account_switched(&mut self, switch: &Switch, result: anyhow::Result<String>, cx: &mut Context<Self>) {
        match switch.answered(result, &switch.agent) {
            Ok(said) => {
                let kind = if said.refused { ToastKind::Error } else { ToastKind::Info };
                self.toast(said.text, kind, cx);
            }
            Err(e) => self.toast(cli::reason(&e), ToastKind::Error, cx),
        }
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
        let (queue, args) = (self.queue.clone(), self.with_view(args));
        self.cmd_busy_on(&queue, busy, args, cx);
    }

    pub(crate) fn cmd_busy_on(&mut self, queue: &Queue, busy: Busy, args: Vec<String>, cx: &mut Context<Self>) {
        let id = self.start_job(busy, cx);
        let (reply, queue) = (reply_of(queue, args), queue.clone());
        cx.spawn(async move |this, cx| {
            let result = reply.await;
            let _ = this.update(cx, |s, cx| s.answered(id, &queue, result.ok(), cx));
        })
        .detach();
    }

    fn answered(&mut self, id: u64, queue: &Queue, answer: Option<anyhow::Result<String>>, cx: &mut Context<Self>) {
        self.end_job(id, cx);
        if let Some(said) = answer_words(queue, answer) {
            self.toast(said, ToastKind::Error, cx);
        }
    }

    pub(crate) fn focus(&self, kind: &str, id: u64) {
        self.act(args(&["focus", &format!("--{kind}"), &id.to_string()]));
    }

    pub(crate) fn show_in_terminal(&mut self, kind: &str, id: u64, cx: &mut Context<Self>) {
        self.focus(kind, id);
        self.give_keys_back(cx);
    }

    pub(crate) fn give_keys_back(&mut self, cx: &mut Context<Self>) {
        self.keys_to_terminal = true;
        self.file_full_screen = false;
        cx.notify();
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
        self.keys_to_terminal = true;
        self.file_full_screen = false;
        self.toast(format!("Sent {label} to {agent}"), ToastKind::Info, cx);
    }

    pub(crate) fn ask_about(&mut self, workspace: u64, reference: &str, cx: &mut Context<Self>) {
        let label = reference.trim_end().to_string();
        self.ask_agent(workspace, reference, &label, cx);
    }

    pub(crate) fn open_in_editor(&self, workspace: u64, path: &str, line: impl Display) {
        let mut a = args(&["new-tab", "--workspace", &workspace.to_string(), "--focus"]);
        if let Some(view) = self.view_id {
            a.extend(["--view".to_string(), view.to_string()]);
        }
        a.push("--".into());
        a.push(editor_command(path, line));
        self.cmd(a);
    }

    pub(crate) fn copy_elsewhere(&mut self, path: String, cx: &mut Context<Self>) {
        self.copy_from(path, &cli::host().unwrap_or_default(), cx);
    }

    pub(crate) fn copy_from(&mut self, path: String, place: &str, cx: &mut Context<Self>) {
        let text = format!("Copied {path}: it is on {place}, not on this {}", desktop::here());
        cx.write_to_clipboard(ClipboardItem::new_string(path));
        self.toast(text, ToastKind::Info, cx);
    }

    pub(crate) fn go(&mut self, action: &ToastGo, cx: &mut Context<Self>) {
        match action {
            ToastGo::Run(args) => self.act(args.clone()),
            ToastGo::Machine(there) => self.go_there(there, cx),
            ToastGo::Browse(url) => {
                let owner = self.local_owner();
                self.open_in_browser(url.clone(), owner, cx);
            }
            ToastGo::Tunnelled(machine, port) => self.open_tunnel(machine, *port, cx),
            ToastGo::Job(busy, args) => self.run_aside(busy.clone(), args.clone(), cx),
            ToastGo::Open(kind) => self.open(*kind, cx),
            ToastGo::Update(updating) => {
                self.asks.open_update = Some(*updating);
                cx.notify();
            }
        }
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
        if cli::moves_view(&args) {
            self.leave_machine_place(cx);
        }
        let id = self.start_job(busy, cx);
        run_job(cx, id, args, |this, result, cx| {
            if let Err(e) = result {
                this.toast(e.to_string(), ToastKind::Error, cx);
            }
        });
    }

    pub(crate) fn run_then<R: Send + 'static>(
        &mut self,
        busy: Busy,
        work: impl FnOnce() -> anyhow::Result<R> + Send + 'static,
        then: impl FnOnce(&mut Self, anyhow::Result<R>, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        let id = self.start_job(busy, cx);
        run_with(cx, Some((cx.entity(), id)), work, then);
    }

    pub(crate) fn worktree_went(&mut self, workspace: u64, cx: &mut Context<Self>) {
        self.jobs.removed(workspace);
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
        let was_shown = self.dock == Some(kind);
        self.dock = if was_shown { None } else { Some(kind) };
        self.shown_again(kind, was_shown, cx);
        cx.notify();
    }

    pub(crate) fn shown_dock(&self) -> Option<PanelKind> {
        self.dock.filter(|kind| kind.shows(self.session.as_ref()))
    }

    pub(crate) fn shows_file_full_screen(&self) -> bool {
        self.file_full_screen && self.shown_dock() == Some(PanelKind::Files)
    }

    pub(crate) fn toggle_file_full_screen(&mut self, cx: &mut Context<Self>) {
        self.file_full_screen = !self.file_full_screen;
        cx.notify();
    }

    pub(crate) fn leave_file_full_screen(&mut self, cx: &mut Context<Self>) -> bool {
        let left = std::mem::take(&mut self.file_full_screen);
        if left {
            cx.notify();
        }
        left
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
        self.peeking = false;
        self.dialog_comes_back();
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
        let was_shown = self.sheet == Some(kind);
        self.sheet = Some(kind);
        self.shown_again(kind, was_shown, cx);
        cx.notify();
    }

    pub(crate) fn open_on_project(&mut self, kind: PanelKind, project: u64, cx: &mut Context<Self>) {
        self.project_sheet = Some((kind, project));
        self.open_sheet(kind, cx);
    }

    pub(crate) fn take_project_sheet(&mut self, kind: PanelKind) -> Option<u64> {
        self.project_sheet.take_if(|(asked, _)| *asked == kind).map(|(_, project)| project)
    }

    pub(crate) fn open(&mut self, kind: PanelKind, cx: &mut Context<Self>) {
        if kind.docked() {
            let was_shown = self.dock == Some(kind);
            self.dock = Some(kind);
            self.shown_again(kind, was_shown, cx);
            cx.notify();
        } else {
            self.open_sheet(kind, cx);
        }
    }

    pub(crate) fn land_on(&mut self, found: FoundSetting, cx: &mut Context<Self>) {
        self.landing = Some(found);
        self.open_sheet(PanelKind::Settings, cx);
    }

    pub(crate) fn read_run_file(&mut self, path: String, cx: &mut Context<Self>) {
        self.leave_machine_place(cx);
        self.run_file = Some(path);
        self.open(PanelKind::Files, cx);
    }

    pub(crate) fn set_pref(&mut self, key: &str, value: &str, cx: &mut Context<Self>) -> anyhow::Result<()> {
        let changed = self.prefs.with(key, value)?;
        prefs::save(&changed)?;
        self.prefs = changed;
        cx.notify();
        Ok(())
    }

    pub(crate) fn toggle_long_lines(&mut self, cx: &mut Context<Self>) {
        let value = if self.prefs.wraps() { "cut" } else { "wrap" };
        if let Err(e) = self.set_pref("long_lines", value, cx) {
            self.toast(format!("{e:#}"), ToastKind::Error, cx);
        }
    }

    pub(crate) fn run_started(&mut self, workspace: u64, cx: &mut Context<Self>) {
        self.started_run_in = Some(workspace);
        cx.notify();
    }

    pub(crate) fn close_sheet(&mut self, cx: &mut Context<Self>) {
        self.sheet = None;
        self.peeking = false;
        cx.notify();
    }

    pub(crate) fn peek(&mut self, on: bool, cx: &mut Context<Self>) {
        if self.peeking != on {
            self.peeking = on;
            cx.notify();
        }
    }

    pub(crate) fn show_dialog(&mut self, view: AnyView, cx: &mut Context<Self>) {
        self.dialog = Some(view);
        cx.notify();
    }

    pub(crate) fn close_dialog(&mut self, cx: &mut Context<Self>) {
        self.dialog = None;
        self.dialog_comes_back();
        cx.notify();
    }

    pub(crate) fn connect(&mut self, target: Target, cx: &mut Context<Self>) {
        self.dialog = None;
        self.dialog_behind = None;
        if target != cli::target() {
            self.switch_to = Some(target);
        }
        cx.notify();
    }
}

fn detect_openers(cx: &mut Context<Store>) {
    if cli::host().is_some() {
        return;
    }
    let found = cx.background_spawn(async { open::detect() });
    cx.spawn(async move |this, cx| {
        let found = Arc::new(found.await);
        let _ = this.update(cx, |this, _| this.openers = found);
    })
    .detach();
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
    let id = start_view_job(cx, busy, &args);
    run_job(cx, id, args, then);
}

fn start_view_job<T: 'static>(cx: &mut Context<T>, busy: Busy, args: &[String]) -> u64 {
    Store::get(cx).update(cx, |s, cx| {
        if cli::moves_view(args) {
            s.leave_machine_place(cx);
        }
        s.start_job(busy, cx)
    })
}

pub(crate) fn run_on<T: 'static>(
    cx: &mut Context<T>,
    target: Target,
    args: Vec<String>,
    then: impl FnOnce(&mut T, anyhow::Result<String>, &mut Context<T>) + 'static,
) {
    run_with(cx, None, move || cli::run_on(&target, &args, None), then);
}

pub(crate) fn run_on_busy<T: 'static>(
    cx: &mut Context<T>,
    busy: Busy,
    target: Target,
    args: Vec<String>,
    then: impl FnOnce(&mut T, anyhow::Result<String>, &mut Context<T>) + 'static,
) {
    let id = Store::get(cx).update(cx, |s, cx| s.start_job(busy, cx));
    run_job_on(cx, id, target, args, then);
}

fn run_job_on<T: 'static>(
    cx: &mut Context<T>,
    id: u64,
    target: Target,
    args: Vec<String>,
    then: impl FnOnce(&mut T, anyhow::Result<String>, &mut Context<T>) + 'static,
) {
    let job = (Store::get(cx), id);
    run_with(cx, Some(job), move || cli::run_on(&target, &args, None), then);
}

pub(crate) fn run_partial_busy<T: 'static>(
    cx: &mut Context<T>,
    busy: Busy,
    args: Vec<String>,
    then: impl FnOnce(&mut T, anyhow::Result<String>, &mut Context<T>) + 'static,
) {
    run_partial_on_busy(cx, busy, cli::target(), args, then);
}

pub(crate) fn run_partial_on_busy<T: 'static>(
    cx: &mut Context<T>,
    busy: Busy,
    target: Target,
    args: Vec<String>,
    then: impl FnOnce(&mut T, anyhow::Result<String>, &mut Context<T>) + 'static,
) {
    let id = start_view_job(cx, busy, &args);
    let job = (Store::get(cx), id);
    run_with(cx, Some(job), move || cli::run_partial_on(&target, &args), then);
}

pub(crate) fn run_local(cx: &mut App, busy: Busy, work: impl FnOnce() -> anyhow::Result<()> + Send + 'static) {
    let store = Store::get(cx);
    let id = store.update(cx, |s, cx| s.start_job(busy, cx));
    let task = cx.background_spawn(async move { work() });
    cx.spawn(async move |cx| {
        let result = task.await;
        let _ = store.update(cx, |s, cx| {
            s.end_job(id, cx);
            if let Err(e) = result {
                s.toast(format!("{e:#}"), ToastKind::Error, cx);
            }
        });
    })
    .detach();
}

pub(crate) fn run_local_busy<T: 'static, R: Send + 'static>(
    cx: &mut Context<T>,
    busy: Busy,
    work: impl FnOnce() -> anyhow::Result<R> + Send + 'static,
    then: impl FnOnce(&mut T, anyhow::Result<R>, &mut Context<T>) + 'static,
) {
    let id = Store::get(cx).update(cx, |s, cx| s.start_job(busy, cx));
    let job = (Store::get(cx), id);
    run_with(cx, Some(job), work, then);
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

fn run_with<T: 'static, R: Send + 'static>(
    cx: &mut Context<T>,
    job: Option<(Entity<Store>, u64)>,
    work: impl FnOnce() -> anyhow::Result<R> + Send + 'static,
    then: impl FnOnce(&mut T, anyhow::Result<R>, &mut Context<T>) + 'static,
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

pub(crate) fn reply(
    cx: &App,
    then: impl FnOnce(anyhow::Result<String>, &mut App) + 'static,
) -> impl FnOnce(anyhow::Result<String>) + Send + 'static {
    handoff(cx, None, then)
}

fn handoff(
    cx: &App,
    job: Option<(Entity<Store>, u64)>,
    then: impl FnOnce(anyhow::Result<String>, &mut App) + 'static,
) -> impl FnOnce(anyhow::Result<String>) + Send + 'static {
    let (tx, rx) = oneshot::channel();
    answered_on(cx, job, rx, then);
    move |result| {
        let _ = tx.send(result);
    }
}

pub(crate) fn queued(cx: &App, args: Vec<String>, then: impl FnOnce(anyhow::Result<String>, &mut App) + 'static) {
    Store::get(cx).read(cx).queue.cmd_then(args, reply(cx, then));
}

pub(crate) fn queued_busy(
    cx: &mut App,
    busy: Busy,
    args: Vec<String>,
    then: impl FnOnce(anyhow::Result<String>, &mut App) + 'static,
) {
    let reply = busy_reply(cx, busy, then);
    Store::get(cx).read(cx).queue.cmd_then(args, reply);
}

pub(crate) fn busy_reply(
    cx: &mut App,
    busy: Busy,
    then: impl FnOnce(anyhow::Result<String>, &mut App) + 'static,
) -> impl FnOnce(anyhow::Result<String>) + Send + 'static {
    let store = Store::get(cx);
    let id = store.update(cx, |s, cx| s.start_job(busy, cx));
    handoff(cx, Some((store, id)), then)
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
    follow_on(cx, &cli::target(), args, on_line, on_end)
}

pub(crate) fn follow_on<T: 'static>(
    cx: &mut Context<T>,
    target: &Target,
    args: impl AsRef<[String]>,
    on_line: impl Fn(&mut T, String, &mut Context<T>) + 'static,
    on_end: impl FnOnce(&mut T, String, &mut Context<T>) + 'static,
) -> Option<Stream> {
    let (tx, rx) = mpsc::unbounded::<Line>();
    let end = tx.clone();
    let stream = cli::stream_on(
        target,
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
