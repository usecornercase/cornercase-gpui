//! Issues and prompts scheduled for later (`cornercase queue`), as the TUI's `Queue` tab: every
//! project's entries, live from `status --app` (its `queue`), chronological, each with its local
//! time, label, state, agent and place. A form beside the list schedules a new entry or edits one
//! that has not started; the rows start an entry now, remove it, or open a started one's pane.
//! Removals cannot be undone yet (usecornercase/cornercase-terminal#463), so a pending one asks.

use std::collections::BTreeMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gpui::{
    AnyElement, AnyView, App, AppContext as _, ClickEvent, Context, Entity, Focusable as _, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Rgba, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, Window, div, prelude::FluentBuilder as _, px,
};
use serde::Deserialize;
use serde_json::Value;

use crate::dialogs;
use crate::model::Session;
use crate::store::{self, Store, ToastKind, args};
use crate::theme::colours;
use crate::ui::{self, TextField, TextFieldEvent};

/// What the queue needs but the command line does not offer yet, said where it is missing.
const UNDO_ISSUE: &str = "usecornercase/cornercase-terminal#463";

/// Where a queued entry starts, as the server recorded it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Place {
    pub project: String,
    pub workspace: Option<String>,
    pub workspace_name: Option<String>,
    pub workspace_id: Option<u64>,
    pub branch: Option<String>,
}

/// An entry of `status --app`'s `queue` (the same as `queue list --json`'s `entries`). Read
/// leniently: a field a newer server adds or drops never hides the entry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Entry {
    pub id: u64,
    /// UTC seconds.
    pub at: i64,
    pub label: String,
    pub agent: String,
    pub mode: Option<String>,
    pub prompt: String,
    pub place: Place,
    /// pending, starting, started, failed or missed.
    pub state: String,
    pub pane: Option<u64>,
    /// The server token a started entry's pane belongs to (`boot` in `status --app`).
    pub server: Option<String>,
    pub reason: Option<String>,
    pub accounts: BTreeMap<String, String>,
}

impl Entry {
    /// Pending, failed and missed entries can be edited and started now.
    pub(crate) fn runnable(&self) -> bool {
        matches!(self.state.as_str(), "pending" | "failed" | "missed")
    }
}

/// The entries in `status --app`, in the server's order (chronological, ties by id).
pub(crate) fn entries(app: Option<&Value>) -> Vec<Entry> {
    app.and_then(|app| app.get("queue"))
        .and_then(Value::as_array)
        .map(|queue| queue.iter().filter_map(|e| Entry::deserialize(e).ok()).filter(|e| e.id != 0).collect())
        .unwrap_or_default()
}

/// The last part of a path, for a project the session no longer has.
fn folder_name(path: &str) -> String {
    path.trim_end_matches('/').rsplit('/').next().unwrap_or(path).to_string()
}

/// `project › workspace`, as the TUI names an entry's place.
pub(crate) fn place_label(place: &Place, session: Option<&Session>) -> String {
    let project = session
        .and_then(|s| s.projects.iter().find(|p| p.path == place.project))
        .map_or_else(|| folder_name(&place.project), |p| p.name.clone());
    match (&place.branch, &place.workspace_name, &place.workspace) {
        (Some(branch), ..) => format!("{project} › {branch}"),
        (None, Some(name), _) => format!("{project} › {name}"),
        (None, None, Some(path)) if *path != place.project => format!("{project} › {}", folder_name(path)),
        _ => project,
    }
}

/// An entry's state as the row says it, and whether a started entry's pane is still there.
pub(crate) fn status_label(entry: &Entry, boot: Option<&str>, session: Option<&Session>) -> String {
    match entry.state.as_str() {
        "starting" => "starting…".into(),
        "started" => match entry.pane {
            Some(pane)
                if boot.is_some()
                    && entry.server.as_deref() == boot
                    && session.and_then(|s| s.pane(pane)).is_some() =>
            {
                format!("started · pane {pane}")
            }
            _ => "started · pane closed".into(),
        },
        "failed" => match &entry.reason {
            Some(reason) => format!("failed · {reason}"),
            None => "failed".into(),
        },
        other => other.to_string(),
    }
}

fn status_colour(state: &str) -> Rgba {
    match state {
        "failed" => colours::failed(),
        "missed" | "starting" => colours::working(),
        "started" => colours::done(),
        _ => colours::accent(),
    }
}

// ---------------------------------------------------------------------------------------------
// Time, in this machine's zone.

/// A local date and time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Civil {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    /// 0 is Sunday.
    pub weekday: u32,
}

impl Civil {
    pub(crate) fn date(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    pub(crate) fn clock(&self) -> String {
        format!("{:02}:{:02}", self.hour, self.minute)
    }
}

/// UTC seconds as this machine's local time.
pub(crate) fn local(at: i64) -> Option<Civil> {
    let t = libc::time_t::try_from(at).ok()?;
    // SAFETY: `localtime_r` only writes the `tm` it is given, which lives on this stack frame.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let ok = unsafe { !libc::localtime_r(&raw const t, &raw mut tm).is_null() };
    ok.then(|| Civil {
        year: tm.tm_year + 1900,
        month: u32::try_from(tm.tm_mon + 1).unwrap_or(1),
        day: u32::try_from(tm.tm_mday).unwrap_or(1),
        hour: u32::try_from(tm.tm_hour).unwrap_or(0),
        minute: u32::try_from(tm.tm_min).unwrap_or(0),
        weekday: u32::try_from(tm.tm_wday).unwrap_or(0),
    })
}

pub(crate) fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

/// Days since 1970-01-01 of a civil date (Howard Hinnant's algorithm).
fn days(year: i32, month: u32, day: u32) -> i64 {
    let y = i64::from(year) - i64::from(month <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(month);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// When an entry runs, read from today: `today 14:30`, `tomorrow 09:00`, `Mon 09:00` within a
/// week, the date otherwise.
pub(crate) fn when_label(at: Civil, today: Civil) -> String {
    const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let clock = at.clock();
    match days(at.year, at.month, at.day) - days(today.year, today.month, today.day) {
        0 => format!("today {clock}"),
        1 => format!("tomorrow {clock}"),
        -1 => format!("yesterday {clock}"),
        2..=6 => format!("{} {clock}", WEEKDAYS.get(at.weekday as usize).copied().unwrap_or("")),
        _ => format!("{} {clock}", at.date()),
    }
}

/// How far an instant is from now: `in 2h 5m`, `3d 4h ago`, `now`.
pub(crate) fn relative(at: i64, now: i64) -> String {
    let delta = at - now;
    let s = delta.unsigned_abs();
    if s < 60 {
        return "now".into();
    }
    let (d, h, m) = (s / 86_400, s % 86_400 / 3600, s % 3600 / 60);
    let text = if d > 0 {
        if h > 0 { format!("{d}d {h}h") } else { format!("{d}d") }
    } else if h > 0 {
        if m > 0 { format!("{h}h {m}m") } else { format!("{h}h") }
    } else {
        format!("{m}m")
    };
    if delta > 0 { format!("in {text}") } else { format!("{text} ago") }
}

// ---------------------------------------------------------------------------------------------
// The form, as plain data: what `queue add` or `queue edit` gets.

/// The kind of time, as the TUI's form asks it first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum When {
    /// Editing: the entry's time stays.
    Keep,
    At,
    Tomorrow,
    On,
    In,
    ClaudeReset,
    CodexReset,
}

impl When {
    const NEW: [When; 6] = [When::In, When::At, When::Tomorrow, When::On, When::ClaudeReset, When::CodexReset];

    fn label(self) -> &'static str {
        match self {
            When::Keep => "unchanged",
            When::At => "at",
            When::Tomorrow => "tomorrow at",
            When::On => "on",
            When::In => "in",
            When::ClaudeReset => "Claude reset",
            When::CodexReset => "Codex reset",
        }
    }
}

/// Where a new entry starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Target {
    /// Editing: the entry's place stays.
    Keep,
    /// A new tab in this workspace.
    Workspace(u64),
    /// A worktree of this project on this branch, made when missing.
    Worktree { project: u64, branch: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Draft {
    /// The entry being edited; a new one otherwise.
    pub edit: Option<u64>,
    /// An issue (URL or key) rather than a prompt.
    pub issue: bool,
    pub text: String,
    pub source: Option<String>,
    pub when: When,
    pub clock: String,
    pub date: String,
    pub amount: String,
    /// `m`, `h` or `d`.
    pub unit: char,
    pub target: Target,
    /// `None` keeps the entry's agent (or uses the default for a new one).
    pub agent: Option<String>,
    pub mode: Option<String>,
    /// Sent when not empty; editing sends it when it changed.
    pub name: Option<String>,
}

impl Draft {
    /// The text `--at` takes, the same the CLI documents (`14:30`, `tomorrow 9:00`,
    /// `2026-10-20 09:00`, `in 2h`, `claude reset`); `None` keeps the entry's time.
    pub(crate) fn at(&self) -> Result<Option<String>, String> {
        let clock = self.clock.trim();
        let need_clock = || if clock.is_empty() { Err("choose a time, such as 14:30".to_string()) } else { Ok(()) };
        Ok(Some(match self.when {
            When::Keep => return Ok(None),
            When::At => {
                need_clock()?;
                clock.to_string()
            }
            When::Tomorrow => {
                need_clock()?;
                format!("tomorrow {clock}")
            }
            When::On => {
                need_clock()?;
                let date = self.date.trim();
                if date.is_empty() {
                    return Err("choose a date, such as 2026-10-24".into());
                }
                format!("{date} {clock}")
            }
            When::In => {
                let amount = self.amount.trim();
                if !amount.parse::<u32>().is_ok_and(|n| n > 0) {
                    return Err("the delay must be a whole number above 0".into());
                }
                format!("in {amount}{}", self.unit)
            }
            When::ClaudeReset => "claude reset".into(),
            When::CodexReset => "codex reset".into(),
        }))
    }

    /// `queue add …` or `queue edit ID …`, with `--json` so the answer can be read.
    pub(crate) fn args(&self) -> Result<Vec<String>, String> {
        let mut a = match self.edit {
            Some(id) => args(&["queue", "edit", &id.to_string()]),
            None => args(&["queue", "add"]),
        };
        let mut push = |flag: &str, value: &str| {
            a.push(flag.to_string());
            a.push(value.to_string());
        };
        if let Some(at) = self.at()? {
            push("--at", &at);
        } else if self.edit.is_none() {
            return Err("choose when it starts".into());
        }
        let text = self.text.trim();
        match (self.issue, text.is_empty()) {
            (true, false) => {
                push("--issue", text);
                if let Some(source) = &self.source {
                    push("--source", source);
                }
            }
            (false, false) => push("--prompt", text),
            (true, true) if self.edit.is_none() => return Err("paste an issue URL or key".into()),
            (false, true) if self.edit.is_none() => return Err("write the prompt it starts with".into()),
            _ => {}
        }
        match &self.target {
            Target::Keep => {}
            Target::Workspace(id) => push("--workspace", &id.to_string()),
            Target::Worktree { project, branch } => {
                if branch.trim().is_empty() {
                    return Err("name the worktree's branch".into());
                }
                push("--project", &project.to_string());
                push("--worktree", branch.trim());
            }
        }
        if let Some(agent) = &self.agent {
            push("--agent", agent);
        }
        if let Some(mode) = &self.mode {
            push("--mode", mode);
        }
        if let Some(name) = self.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
            push("--name", name);
        }
        a.push("--json".into());
        Ok(a)
    }
}

/// The agents the settings offer (`config --json`, setting `agent`) and the default one.
pub(crate) fn agent_choices(config: &Value) -> (Vec<String>, String) {
    let setting = setting(config, "agent");
    let choices = setting.map(choice_values).unwrap_or_default();
    let default = setting.and_then(|s| s.get("value")).and_then(Value::as_str).unwrap_or("auto").to_string();
    (choices, default)
}

/// An agent's modes (`agents.NAME.mode`), and the one chosen in the settings.
pub(crate) fn mode_choices(config: &Value, agent: &str) -> (Vec<String>, Option<String>) {
    let setting = setting(config, &format!("agents.{agent}.mode"));
    let choices = setting.map(choice_values).unwrap_or_default();
    let chosen = setting.and_then(|s| s.get("value")).and_then(Value::as_str).map(str::to_string);
    (choices, chosen)
}

fn setting<'a>(config: &'a Value, key: &str) -> Option<&'a Value> {
    config.get("settings")?.as_array()?.iter().find(|s| s.get("key").and_then(Value::as_str) == Some(key))
}

fn choice_values(setting: &Value) -> Vec<String> {
    setting
        .get("choices")
        .and_then(Value::as_array)
        .map(|c| c.iter().filter_map(|c| c.get("value").and_then(Value::as_str)).map(str::to_string).collect())
        .unwrap_or_default()
}

const SOURCES: [&str; 7] = ["github", "gitlab", "shortcut", "linear", "jira", "plane", "asana"];

// ---------------------------------------------------------------------------------------------
// The panel.

/// The form's fields and choices, while it is open.
struct Form {
    edit: Option<u64>,
    issue: bool,
    prompt: Entity<TextField>,
    issue_key: Entity<TextField>,
    source: Option<String>,
    when: When,
    clock: Entity<TextField>,
    date: Entity<TextField>,
    amount: Entity<TextField>,
    unit: char,
    project: Option<u64>,
    /// `None` keeps the entry's place (editing); `Some(None)` is a new worktree.
    workspace: Option<Option<u64>>,
    branch: Entity<TextField>,
    /// `None` keeps the entry's agent (editing) or uses the default.
    agent: Option<String>,
    mode: Option<String>,
    name: Entity<TextField>,
    old_name: String,
    old_prompt: String,
    error: Option<String>,
    busy: bool,
    _subscriptions: Vec<Subscription>,
}

pub(crate) struct QueuePanel {
    filter: Entity<TextField>,
    form: Option<Form>,
    /// `config --json`, read when the form first opens, for the agents and their modes.
    config: Option<Value>,
    _subscriptions: Vec<Subscription>,
}

impl QueuePanel {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| TextField::new("Filter by label, prompt, agent or place", cx));
        let subscriptions = vec![
            cx.observe(&Store::get(cx), |_, _, cx| cx.notify()),
            cx.subscribe_in(&filter, window, |_, field, event, _, cx| match event {
                TextFieldEvent::Cancel => field.update(cx, TextField::clear),
                _ => cx.notify(),
            }),
        ];
        // Relative times ("in 5m") move on by themselves.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(30)).await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        })
        .detach();
        Self { filter, form: None, config: None, _subscriptions: subscriptions }
    }

    fn field(
        &self,
        placeholder: &str,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
        subscriptions: &mut Vec<Subscription>,
    ) -> Entity<TextField> {
        let field = cx.new(|cx| {
            let mut field = TextField::new(placeholder.to_string(), cx);
            if !text.is_empty() {
                field.set_text(text.to_string(), cx);
            }
            field
        });
        subscriptions.push(cx.subscribe_in(&field, window, |this, _, event, _, cx| match event {
            TextFieldEvent::Submit => this.save(cx),
            TextFieldEvent::Cancel => this.close_form(cx),
            TextFieldEvent::Changed => cx.notify(),
        }));
        field
    }

    /// Opens the form for a new entry, or for editing one that has not started.
    fn open_form(&mut self, entry: Option<&Entry>, window: &mut Window, cx: &mut Context<Self>) {
        if self.config.is_none() {
            store::run_json(cx, args(&["config", "--json"]), |this: &mut Self, config, cx| {
                if let Ok(config) = config {
                    this.config = Some(config);
                    cx.notify();
                }
            });
        }
        let session = Store::get(cx).read(cx).session.clone();
        let shown = session.as_ref().map(Session::shown).unwrap_or_default();
        let at = entry.and_then(|e| local(e.at));
        let mut subs = Vec::new();
        let prompt = self.field("What the agent should do", entry.map_or("", |e| &e.prompt), window, cx, &mut subs);
        let issue_key = self.field("https://github.com/org/repo/issues/12, #12 or ENG-42", "", window, cx, &mut subs);
        let clock = self.field("14:30", &at.map(|c| c.clock()).unwrap_or_default(), window, cx, &mut subs);
        let date = self.field("2026-10-24", &at.map(|c| c.date()).unwrap_or_default(), window, cx, &mut subs);
        let amount = self.field("1", if entry.is_some() { "" } else { "1" }, window, cx, &mut subs);
        let branch = self.field("feat/my-branch", "", window, cx, &mut subs);
        let name = self.field("the tab's name (optional)", entry.map_or("", |e| &e.label), window, cx, &mut subs);
        let focus = prompt.read(cx).focus_handle(cx);
        // A new entry starts where the window is; an edited one keeps its place.
        let (project, workspace) = match entry {
            Some(e) => {
                let project = session.as_ref().and_then(|s| s.projects.iter().find(|p| p.path == e.place.project));
                (project.map(|p| p.id), None)
            }
            None => (shown.project, Some(shown.workspace)),
        };
        self.form = Some(Form {
            edit: entry.map(|e| e.id),
            issue: false,
            prompt,
            issue_key,
            source: None,
            when: if entry.is_some() { When::Keep } else { When::In },
            clock,
            date,
            amount,
            unit: 'h',
            project,
            workspace,
            branch,
            agent: None,
            mode: None,
            name,
            old_name: entry.map(|e| e.label.clone()).unwrap_or_default(),
            old_prompt: entry.map(|e| e.prompt.clone()).unwrap_or_default(),
            error: None,
            busy: false,
            _subscriptions: subs,
        });
        window.focus(&focus);
        cx.notify();
    }

    fn close_form(&mut self, cx: &mut Context<Self>) {
        self.form = None;
        cx.notify();
    }

    /// The form as a `Draft`, sending only what an edit changed.
    fn draft(&self, cx: &App) -> Option<Draft> {
        let form = self.form.as_ref()?;
        let text = |f: &Entity<TextField>| f.read(cx).text();
        let editing = form.edit.is_some();
        let mut body = if form.issue { text(&form.issue_key) } else { text(&form.prompt) };
        if editing && !form.issue && body.trim() == form.old_prompt.trim() {
            body.clear();
        }
        let name = text(&form.name);
        let name = if editing && name.trim() == form.old_name.trim() { None } else { Some(name) };
        let target = match (form.workspace, form.project) {
            (None, _) => Target::Keep,
            (Some(Some(id)), _) => Target::Workspace(id),
            (Some(None), Some(project)) => Target::Worktree { project, branch: text(&form.branch) },
            (Some(None), None) => Target::Keep,
        };
        Some(Draft {
            edit: form.edit,
            issue: form.issue,
            text: body,
            source: form.source.clone(),
            when: form.when,
            clock: text(&form.clock),
            date: text(&form.date),
            amount: text(&form.amount),
            unit: form.unit,
            target,
            agent: form.agent.clone(),
            mode: form.mode.clone(),
            name,
        })
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let Some(draft) = self.draft(cx) else { return };
        let Some(form) = self.form.as_mut() else { return };
        if form.busy {
            return;
        }
        let command = match draft.args() {
            Ok(command) => command,
            Err(e) => {
                form.error = Some(e);
                cx.notify();
                return;
            }
        };
        form.busy = true;
        form.error = None;
        let editing = draft.edit.is_some();
        store::run_json(cx, command, move |this: &mut Self, answer, cx| match answer {
            Ok(_) => {
                this.form = None;
                let text = if editing { "Queue entry saved" } else { "Scheduled" };
                Store::get(cx).update(cx, |s, cx| s.toast(text, ToastKind::Done, cx));
                cx.notify();
            }
            Err(e) => {
                if let Some(form) = this.form.as_mut() {
                    form.busy = false;
                    let text = e.to_string();
                    form.error = Some(text.strip_prefix("cornercase queue: ").unwrap_or(&text).to_string());
                }
                cx.notify();
            }
        });
        cx.notify();
    }

    fn run_now(&mut self, id: u64, cx: &mut Context<Self>) {
        store::run(cx, args(&["queue", "run", &id.to_string()]), |_: &mut Self, result, cx| {
            let (text, kind) = match result {
                Ok(_) => ("Starting the queued entry".to_string(), ToastKind::Info),
                Err(e) => (e.to_string(), ToastKind::Error),
            };
            Store::get(cx).update(cx, |s, cx| s.toast(text, kind, cx));
        });
    }

    fn remove(&mut self, entry: &Entry, window: &mut Window, cx: &mut Context<Self>) {
        let command = args(&["queue", "remove", &entry.id.to_string()]);
        if self.form.as_ref().is_some_and(|f| f.edit == Some(entry.id)) {
            self.form = None;
        }
        if entry.state == "started" {
            // History only: removing it does not stop the agent.
            Store::get(cx).read(cx).cmd(command);
            return;
        }
        dialogs::confirm(
            dialogs::Question::new(
                "Remove this queue entry?",
                format!(
                    "“{}” will not start, and its prompt and choices are gone: undoing a removal needs {UNDO_ISSUE}.",
                    entry.label
                ),
                "Remove",
                command,
            ),
            window,
            cx,
        );
    }

    // -----------------------------------------------------------------------------------------
    // Drawing.

    fn render_entry(&self, entry: &Entry, ctx: &RowContext, cx: &mut Context<Self>) -> AnyElement {
        let id = entry.id;
        let editing = self.form.as_ref().is_some_and(|f| f.edit == Some(id));
        let when = local(entry.at).map_or_else(|| entry.at.to_string(), |at| when_label(at, ctx.today));
        let status = status_label(entry, ctx.boot.as_deref(), ctx.session.as_ref());
        let mut meta = vec![entry.agent.clone()];
        if let Some(mode) = entry.mode.as_ref().filter(|m| *m != "default") {
            meta.push(mode.clone());
        }
        meta.push(place_label(&entry.place, ctx.session.as_ref()));
        for (agent, account) in &entry.accounts {
            meta.push(format!("{agent}: {}", if account.is_empty() { "System" } else { account }));
        }
        let first_line = entry.prompt.lines().next().unwrap_or_default().to_string();

        let mut buttons = div().flex_none().flex().items_center().gap(px(6.));
        if entry.runnable() {
            let edit_entry = entry.clone();
            buttons = buttons
                .child(ui::button(("queue-edit", id), "Edit").on_click(cx.listener(
                    move |this, _: &ClickEvent, window, cx| {
                        this.open_form(Some(&edit_entry), window, cx);
                    },
                )))
                .child(
                    ui::primary_button(("queue-run", id), "Start now")
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.run_now(id, cx))),
                );
        }
        let pane = entry.pane.filter(|_| status.starts_with("started · pane ") && !status.ends_with("closed"));
        if let Some(pane) = pane {
            buttons =
                buttons.child(ui::primary_button(("queue-open", id), "Open").on_click(move |_: &ClickEvent, _, cx| {
                    Store::get(cx).update(cx, |s, cx| {
                        s.focus("pane", pane);
                        s.close_sheet(cx);
                    });
                }));
        }
        if entry.state != "starting" {
            let removed = entry.clone();
            buttons = buttons.child(
                ui::icon_button(("queue-rm", id), "✕")
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.remove(&removed, window, cx))),
            );
        }

        div()
            .id(("queue", id))
            .flex()
            .items_start()
            .gap(px(14.))
            .px(px(12.))
            .py(px(9.))
            .rounded(px(6.))
            .border_1()
            .border_color(if editing { colours::accent() } else { gpui::transparent_black().into() })
            .hover(|s| s.bg(colours::hover()))
            .child(
                div()
                    .w(px(130.))
                    .flex_none()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(div().text_size(px(13.)).text_color(colours::text()).child(when))
                    .child(ui::dim(relative(entry.at, ctx.now))),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .child(div().text_size(px(13.)).truncate().child(entry.label.clone()))
                    .when(!first_line.is_empty() && first_line != entry.label, |d| {
                        d.child(div().text_size(px(12.)).text_color(colours::muted()).truncate().child(first_line))
                    })
                    .child(
                        div()
                            .flex()
                            .gap(px(6.))
                            .text_size(px(11.))
                            .child(div().flex_none().text_color(status_colour(&entry.state)).child(status))
                            .child(
                                div().text_color(colours::faint()).truncate().child(format!("· {}", meta.join(" · "))),
                            ),
                    ),
            )
            .child(buttons)
            .into_any_element()
    }

    fn render_form(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let form = self.form.as_ref()?;
        let session = Store::get(cx).read(cx).session.clone();
        let boxed = |field: &Entity<TextField>, window: &Window, cx: &App| {
            ui::input_box(field.clone(), field.read(cx).focus_handle(cx).is_focused(window))
        };
        let editing = form.edit.is_some();

        // What: a prompt or an issue.
        let what = choice_row(vec![
            seg("q-what-prompt", "Prompt", !form.issue, cx, |f| f.issue = false),
            seg("q-what-issue", "Issue", form.issue, cx, |f| f.issue = true),
        ]);
        let body: AnyElement = if form.issue {
            let mut sources = vec![seg("q-src-auto", "auto", form.source.is_none(), cx, |f| f.source = None)];
            for source in SOURCES {
                sources.push(seg(
                    SharedString::from(format!("q-src-{source}")),
                    source,
                    form.source.as_deref() == Some(source),
                    cx,
                    move |f| {
                        f.source = Some(source.to_string());
                    },
                ));
            }
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(boxed(&form.issue_key, window, cx))
                .child(ui::field("Tracker, for an ambiguous key or a self-hosted URL", wrap_row(sources)))
                .into_any_element()
        } else {
            boxed(&form.prompt, window, cx).into_any_element()
        };

        // When: the kind, then its values.
        let mut kinds = Vec::new();
        if editing {
            kinds.push(seg("q-when-keep", When::Keep.label(), form.when == When::Keep, cx, |f| f.when = When::Keep));
        }
        for kind in When::NEW {
            kinds.push(seg(("q-when", kind as usize), kind.label(), form.when == kind, cx, move |f| f.when = kind));
        }
        let values: Option<AnyElement> = match form.when {
            When::At | When::Tomorrow => {
                Some(div().w(px(110.)).child(boxed(&form.clock, window, cx)).into_any_element())
            }
            When::On => Some(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(div().w(px(130.)).child(boxed(&form.date, window, cx)))
                    .child(div().w(px(90.)).child(boxed(&form.clock, window, cx)))
                    .into_any_element(),
            ),
            When::In => Some(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(div().w(px(70.)).child(boxed(&form.amount, window, cx)))
                    .child(choice_row(
                        [('m', "minutes"), ('h', "hours"), ('d', "days")]
                            .into_iter()
                            .map(|(unit, label)| {
                                seg(("q-unit", unit as usize), label, form.unit == unit, cx, move |f| f.unit = unit)
                            })
                            .collect(),
                    ))
                    .into_any_element(),
            ),
            When::ClaudeReset | When::CodexReset => {
                Some(ui::dim("when the plan's 5-hour session resets, read once now").into_any_element())
            }
            When::Keep => None,
        };

        // Where: a project, then a workspace of it or a new worktree.
        let mut place = div().flex().flex_col().gap(px(8.));
        if editing {
            place = place.child(choice_row(vec![
                seg("q-place-keep", "unchanged", form.workspace.is_none(), cx, |f| f.workspace = None),
                seg("q-place-change", "somewhere else", form.workspace.is_some(), cx, |f| {
                    f.workspace = Some(None);
                }),
            ]));
        }
        if form.workspace.is_some() {
            let projects: Vec<(u64, String)> = session
                .as_ref()
                .map(|s| s.projects.iter().map(|p| (p.id, p.name.clone())).collect())
                .unwrap_or_default();
            place = place.child(wrap_row(
                projects
                    .into_iter()
                    .map(|(id, name)| {
                        seg(("q-project", id as usize), name, form.project == Some(id), cx, move |f| {
                            f.project = Some(id);
                            f.workspace = Some(None);
                        })
                    })
                    .collect(),
            ));
            let workspaces: Vec<(u64, String)> = session
                .as_ref()
                .and_then(|s| s.projects.iter().find(|p| Some(p.id) == form.project))
                .map(|p| p.workspaces.iter().map(|w| (w.id, w.name.clone())).collect())
                .unwrap_or_default();
            if form.project.is_some() {
                let mut choices: Vec<AnyElement> = workspaces
                    .into_iter()
                    .map(|(id, name)| {
                        seg(("q-ws", id as usize), name, form.workspace == Some(Some(id)), cx, move |f| {
                            f.workspace = Some(Some(id));
                        })
                    })
                    .collect();
                choices.push(seg("q-ws-new", "+ new worktree", form.workspace == Some(None), cx, |f| {
                    f.workspace = Some(None);
                }));
                place = place.child(ui::field("A new tab in", wrap_row(choices)));
                if form.workspace == Some(None) {
                    place = place.child(ui::field("Branch, made when missing", boxed(&form.branch, window, cx)));
                }
            }
        }

        // Agent and mode, from the settings.
        let (agents, default_agent) = self.config.as_ref().map(agent_choices).unwrap_or_default();
        let agent_now = form.agent.clone();
        let mut agent_segs = Vec::new();
        if editing {
            agent_segs.push(seg("q-agent-keep", "unchanged", agent_now.is_none(), cx, |f| {
                f.agent = None;
                f.mode = None;
            }));
        }
        for agent in agents {
            let selected = match &agent_now {
                Some(a) => *a == agent,
                None => !editing && agent == default_agent,
            };
            let value = agent.clone();
            agent_segs.push(seg(SharedString::from(format!("q-agent-{agent}")), agent, selected, cx, move |f| {
                f.agent = Some(value.clone());
                f.mode = None;
            }));
        }
        let mode_agent = agent_now.clone().or_else(|| (!editing).then(|| default_agent.clone()));
        let modes = match (&self.config, mode_agent.as_deref()) {
            (Some(config), Some(agent)) if agent != "auto" => Some(mode_choices(config, agent)),
            _ => None,
        };
        let mode_row = modes.filter(|(m, _)| !m.is_empty()).map(|(modes, chosen)| {
            let mut segs = vec![seg("q-mode-default", "as in settings", form.mode.is_none(), cx, |f| f.mode = None)];
            for mode in modes {
                let value = mode.clone();
                let label = if Some(&mode) == chosen.as_ref() { format!("{mode} ✓") } else { mode.clone() };
                segs.push(seg(
                    SharedString::from(format!("q-mode-{mode}")),
                    label,
                    form.mode.as_ref() == Some(&mode),
                    cx,
                    move |f| {
                        f.mode = Some(value.clone());
                    },
                ));
            }
            ui::field("Mode", wrap_row(segs))
        });

        let buttons = div()
            .flex()
            .justify_end()
            .gap(px(8.))
            .child(
                ui::button("q-cancel", "Cancel")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_form(cx))),
            )
            .child(
                ui::primary_button(
                    "q-save",
                    match (form.busy, editing) {
                        (true, _) => "Saving…",
                        (false, true) => "Save",
                        (false, false) => "Schedule",
                    },
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.save(cx))),
            );

        Some(
            div()
                .id("queue-form")
                .w(px(420.))
                .flex_none()
                .h_full()
                .overflow_y_scroll()
                .border_l_1()
                .border_color(colours::border())
                .bg(colours::sidebar())
                .p(px(16.))
                .flex()
                .flex_col()
                .gap(px(14.))
                .child(ui::title(if editing { "Edit queue entry" } else { "Schedule for later" }))
                .child(ui::field("What", div().flex().flex_col().gap(px(8.)).child(what).child(body)))
                .child(ui::field("When", div().flex().flex_col().gap(px(8.)).child(wrap_row(kinds)).children(values)))
                .child(ui::field("Where", place))
                .child(ui::field(
                    "Agent",
                    if self.config.is_some() {
                        wrap_row(agent_segs).into_any_element()
                    } else {
                        ui::dim("Loading…").into_any_element()
                    },
                ))
                .children(mode_row)
                .child(ui::field("Name", boxed(&form.name, window, cx)))
                .child(ui::dim(
                    "Queued agents accept the folder's trust prompt, and the prompt is submitted by itself.",
                ))
                .when_some(form.error.clone(), |d, e| {
                    d.child(div().text_size(px(12.)).text_color(colours::failed()).child(e))
                })
                .child(buttons)
                .into_any_element(),
        )
    }
}

/// What every row needs, read once per frame.
struct RowContext {
    session: Option<Session>,
    boot: Option<String>,
    today: Civil,
    now: i64,
}

/// A segment that changes the form when clicked.
fn seg(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    cx: &mut Context<QueuePanel>,
    change: impl Fn(&mut Form) + 'static,
) -> AnyElement {
    ui::segment(id, label, selected)
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            if let Some(form) = this.form.as_mut() {
                change(form);
                form.error = None;
                cx.notify();
            }
        }))
        .into_any_element()
}

fn choice_row(items: Vec<AnyElement>) -> gpui::Div {
    ui::segments(items)
}

/// Segments that wrap onto more lines when they do not fit.
fn wrap_row(items: Vec<AnyElement>) -> gpui::Div {
    ui::segments(items).flex_wrap()
}

impl Render for QueuePanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let store = Store::get(cx);
        let (app, session) = {
            let s = store.read(cx);
            (s.app.clone(), s.session.clone())
        };
        let entries = entries(app.as_ref());
        // An entry that started or went away meanwhile cannot be edited any more.
        if let Some(edit) = self.form.as_ref().and_then(|f| f.edit)
            && !entries.iter().any(|e| e.id == edit && e.runnable())
        {
            self.form = None;
        }
        // A new entry starts where the window is, once the session says where that is.
        if let Some(form) = self.form.as_mut()
            && form.edit.is_none()
            && form.project.is_none()
            && let Some(shown) = session.as_ref().map(Session::shown)
        {
            form.project = shown.project;
            form.workspace = Some(shown.workspace);
        }
        let now = now();
        let ctx = RowContext {
            session,
            boot: app.as_ref().and_then(|a| a.get("boot")).and_then(Value::as_str).map(str::to_string),
            today: local(now).unwrap_or(Civil { year: 1970, month: 1, day: 1, hour: 0, minute: 0, weekday: 4 }),
            now,
        };
        let filter = self.filter.read(cx).text().to_lowercase();
        let shown: Vec<&Entry> = entries
            .iter()
            .filter(|e| {
                filter.is_empty() || {
                    let place = place_label(&e.place, ctx.session.as_ref());
                    [&e.label, &e.prompt, &e.agent, &place].iter().any(|t| t.to_lowercase().contains(&filter))
                }
            })
            .collect();
        let started: Vec<u64> = entries.iter().filter(|e| e.state == "started").map(|e| e.id).collect();
        let pending = entries.iter().filter(|e| e.state == "pending").count();

        let focused = self.filter.read(cx).focus_handle(cx).is_focused(window);
        let toolbar = div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(10.))
            .px(px(16.))
            .py(px(10.))
            .border_b_1()
            .border_color(colours::border())
            .child(div().flex_1().child(ui::input_box(self.filter.clone(), focused)))
            .child(ui::dim(format!("{pending} pending · {} in all", entries.len())))
            .when(!started.is_empty(), |d| {
                let started = started.clone();
                d.child(ui::button("queue-clear-started", "Clear started").on_click(move |_: &ClickEvent, _, cx| {
                    // History only: started agents keep running. One `remove` each until the
                    // command line has a `clear-started` (#463).
                    let store = Store::get(cx);
                    for id in &started {
                        store.read(cx).cmd(args(&["queue", "remove", &id.to_string()]));
                    }
                }))
            })
            .child(
                ui::primary_button("queue-new", "+ New")
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.open_form(None, window, cx))),
            );

        let mut list = div().flex().flex_col().gap(px(2.)).p(px(10.));
        if app.is_none() {
            list = list.child(ui::note("Loading…"));
        } else if entries.is_empty() {
            list = list.child(ui::note(
                "Nothing is scheduled. + New queues a prompt or an issue to start an agent later, even unattended.",
            ));
        } else if shown.is_empty() {
            list = list.child(ui::note("No entry matches the filter."));
        }
        for entry in shown {
            list = list.child(self.render_entry(entry, &ctx, cx));
        }
        if !entries.is_empty() {
            list = list.child(div().px(px(12.)).pt(px(8.)).child(ui::dim(format!(
                "Removing an entry cannot be undone yet ({UNDO_ISSUE}); removing a started one does not stop its agent."
            ))));
        }

        let form = self.render_form(window, cx);
        div()
            .size_full()
            .flex()
            .text_color(colours::text())
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(toolbar)
                    .child(div().id("queue-list").flex_1().overflow_y_scroll().child(list)),
            )
            .children(form)
    }
}

pub(crate) fn build(window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|cx| QueuePanel::new(window, cx)).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_entries_leniently() {
        let app = json!({ "boot": "1-2", "queue": [
            { "id": 4, "at": 1_800_000_000, "zone": "Europe/Madrid", "label": "#12 Fix login", "agent": "claude",
              "command": "claude", "program": "claude", "mode": "plan", "prompt": "Fix it\nplease", "submit": true,
              "place": { "project": "/src/shop", "workspace": "/src/shop", "workspace_name": null,
                         "workspace_id": 3, "server": "1-2", "branch": "fix-login" },
              "state": "started", "pane": 9, "server": "1-2" },
            { "id": 5, "at": 1_800_000_100, "label": "later", "state": "failed", "reason": "agent missing",
              "accounts": { "claude": "" }, "new_field": 1 },
            "not an entry",
        ]});
        let list = entries(Some(&app));
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].place.branch.as_deref(), Some("fix-login"));
        assert_eq!(list[0].pane, Some(9));
        assert_eq!(list[0].server.as_deref(), Some("1-2"));
        assert_eq!(list[1].reason.as_deref(), Some("agent missing"));
        assert!(list[1].runnable() && !list[0].runnable());
        assert_eq!(status_label(&list[1], None, None), "failed · agent missing");
        assert_eq!(status_label(&list[0], Some("other"), None), "started · pane closed");
        assert!(entries(None).is_empty());
    }

    #[test]
    fn names_places_like_the_tui() {
        assert_eq!(folder_name("/src/shop/"), "shop");
        let mut place = Place { project: "/src/shop".into(), ..Default::default() };
        assert_eq!(place_label(&place, None), "shop");
        place.workspace = Some("/src/shop".into());
        assert_eq!(place_label(&place, None), "shop");
        place.workspace = Some("/src/wt/feat-x".into());
        assert_eq!(place_label(&place, None), "shop › feat-x");
        place.workspace_name = Some("review".into());
        assert_eq!(place_label(&place, None), "shop › review");
        place.branch = Some("feat/x".into());
        assert_eq!(place_label(&place, None), "shop › feat/x");
    }

    fn civil(year: i32, month: u32, day: u32, weekday: u32) -> Civil {
        Civil { year, month, day, hour: 9, minute: 5, weekday }
    }

    #[test]
    fn says_when_from_today() {
        let today = civil(2026, 10, 31, 6);
        assert_eq!(when_label(civil(2026, 10, 31, 6), today), "today 09:05");
        assert_eq!(when_label(civil(2026, 11, 1, 0), today), "tomorrow 09:05");
        assert_eq!(when_label(civil(2026, 10, 30, 5), today), "yesterday 09:05");
        assert_eq!(when_label(civil(2026, 11, 3, 2), today), "Tue 09:05");
        assert_eq!(when_label(civil(2027, 1, 2, 6), today), "2027-01-02 09:05");
        assert_eq!(days(1970, 1, 1), 0);
        assert_eq!(days(2000, 3, 1), 11_017);
    }

    #[test]
    fn says_how_far() {
        assert_eq!(relative(1000, 1000), "now");
        assert_eq!(relative(1000 + 300, 1000), "in 5m");
        assert_eq!(relative(1000 + 7500, 1000), "in 2h 5m");
        assert_eq!(relative(1000 + 7200, 1000), "in 2h");
        assert_eq!(relative(1000, 1000 + 90_000), "1d 1h ago");
    }

    #[test]
    fn local_time_round_trips_the_date() {
        let at = local(1_800_000_000).expect("localtime");
        assert!((2027..=2027).contains(&at.year));
        assert!(at.hour < 24 && at.minute < 60 && at.weekday < 7);
    }

    fn draft() -> Draft {
        Draft {
            edit: None,
            issue: false,
            text: "review the login fix".into(),
            source: None,
            when: When::In,
            clock: String::new(),
            date: String::new(),
            amount: "2".into(),
            unit: 'h',
            target: Target::Workspace(7),
            agent: None,
            mode: None,
            name: Some(String::new()),
        }
    }

    #[test]
    fn builds_queue_add() {
        let d = draft();
        assert_eq!(
            d.args().map(|a| a.join(" ")),
            Ok("queue add --at in 2h --prompt review the login fix --workspace 7 --json".to_string())
        );
        let d = Draft {
            issue: true,
            text: "ENG-42".into(),
            source: Some("linear".into()),
            when: When::On,
            date: "2026-10-24".into(),
            clock: "9:00".into(),
            target: Target::Worktree { project: 3, branch: "feat/x".into() },
            agent: Some("codex".into()),
            mode: Some("plan".into()),
            name: Some("night run".into()),
            ..draft()
        };
        assert_eq!(
            d.args().map(|a| a.join(" ")),
            Ok("queue add --at 2026-10-24 9:00 --issue ENG-42 --source linear --project 3 --worktree feat/x \
             --agent codex --mode plan --name night run --json"
                .to_string())
        );
        assert_eq!(
            Draft { when: When::Tomorrow, clock: "9:30".into(), ..draft() }.at(),
            Ok(Some("tomorrow 9:30".into()))
        );
        assert_eq!(Draft { when: When::ClaudeReset, ..draft() }.at(), Ok(Some("claude reset".into())));
    }

    #[test]
    fn refuses_an_incomplete_form() {
        assert!(Draft { text: " ".into(), ..draft() }.args().is_err());
        assert!(Draft { amount: "0".into(), ..draft() }.args().is_err());
        assert!(Draft { when: When::At, ..draft() }.args().is_err());
        assert!(Draft { target: Target::Worktree { project: 1, branch: " ".into() }, ..draft() }.args().is_err());
    }

    #[test]
    fn an_edit_sends_only_what_changed() {
        let d =
            Draft { edit: Some(4), text: String::new(), when: When::Keep, target: Target::Keep, name: None, ..draft() };
        assert_eq!(d.args().map(|a| a.join(" ")), Ok("queue edit 4 --json".to_string()));
        let d = Draft { edit: Some(4), when: When::At, clock: "14:30".into(), target: Target::Keep, ..draft() };
        assert_eq!(
            d.args().map(|a| a.join(" ")),
            Ok("queue edit 4 --at 14:30 --prompt review the login fix --json".to_string())
        );
    }

    #[test]
    fn reads_agents_and_modes_from_the_settings() {
        let config = json!({ "settings": [
            { "key": "agent", "value": "claude", "choices": [{ "value": "auto" }, { "value": "claude" }] },
            { "key": "agents.claude.mode", "value": "plan", "choices": [{ "value": "default" }, { "value": "plan" }] },
        ]});
        assert_eq!(agent_choices(&config), (vec!["auto".to_string(), "claude".to_string()], "claude".to_string()));
        assert_eq!(mode_choices(&config, "claude"), (vec!["default".into(), "plan".into()], Some("plan".into())));
        assert_eq!(mode_choices(&config, "codex"), (vec![], None));
        assert_eq!(agent_choices(&json!({})), (vec![], "auto".to_string()));
    }
}
