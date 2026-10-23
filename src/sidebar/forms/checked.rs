use gpui::{
    App, AppContext as _, ClickEvent, Context, Div, FocusHandle, Focusable, IntoElement, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _, px,
};
use serde_json::Value;

use super::{Check, Checked, bullet, buttons, keyed, line, load_check, show, warning};
use crate::app::tilde;
use crate::dialogs::frame;
use crate::store::{Store, args};
use crate::theme::colours;
use crate::ui;

pub(in crate::sidebar) struct RemoveWorktree {
    workspace: u64,
    name: String,
    check: Check,
    force: bool,
    unlock: bool,
    focus: FocusHandle,
}

pub(in crate::sidebar) fn remove_worktree(workspace: u64, name: String, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| {
        load_check(
            args(&["close", "--workspace", &workspace.to_string(), "--remove-worktree", "--check", "--json"]),
            cx,
        );
        RemoveWorktree { workspace, name, check: Check::Loading, force: false, unlock: false, focus: cx.focus_handle() }
    });
    show(view, window, cx);
}

fn text_of(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

struct Lock {
    reason: String,
    running: Option<bool>,
}

struct WorktreeCheck {
    path: String,
    branch: String,
    changed: bool,
    lock: Option<Lock>,
    roles: Vec<String>,
    teardown: bool,
}

impl WorktreeCheck {
    fn of(v: &Value) -> Self {
        let lock = v.get("lock").filter(|l| !l.is_null()).map(|lock| Lock {
            reason: lock.get("reason").and_then(Value::as_str).unwrap_or("no reason given").to_string(),
            running: lock.get("running").and_then(Value::as_bool),
        });
        let roles = v.get("roles").and_then(Value::as_array).into_iter().flatten();
        WorktreeCheck {
            path: text_of(v, "path"),
            branch: text_of(v, "branch"),
            changed: v.get("changed").and_then(Value::as_bool).unwrap_or(false),
            lock,
            roles: roles.map(|r| format!("{} · {}", text_of(r, "role"), text_of(r, "branch"))).collect(),
            teardown: v.get("teardown").and_then(Value::as_bool).unwrap_or(false),
        }
    }
}

impl Lock {
    fn text(&self) -> String {
        let running = match self.running {
            Some(true) => ", and the process holding it still runs",
            Some(false) => ", by a process that no longer runs",
            None => "",
        };
        format!("Locked: {}{running}.", self.reason)
    }
}

impl RemoveWorktree {
    fn ready(&self) -> bool {
        let Check::Ready(v) = &self.check else { return false };
        let changed = v.get("changed").and_then(Value::as_bool).unwrap_or(false);
        let locked = v.get("lock").is_some_and(|l| !l.is_null());
        (!changed || self.force) && (!locked || self.unlock)
    }

    fn run(&mut self, cx: &mut Context<Self>) {
        if !self.ready() {
            return;
        }
        let mut a = args(&["close", "--workspace", &self.workspace.to_string(), "--remove-worktree"]);
        if self.force {
            a.push("--force".into());
        }
        if self.unlock {
            a.push("--unlock".into());
        }
        Store::get(cx).update(cx, |s, cx| {
            s.cmd(a);
            s.close_dialog(cx);
        });
    }

    fn report(&self, check: WorktreeCheck, cx: &mut Context<Self>) -> Div {
        let force = ui::checkbox("force", self.force, "Delete them anyway").on_click(cx.listener(
            |this, _: &ClickEvent, _, cx| {
                this.force = !this.force;
                cx.notify();
            },
        ));
        let unlock = ui::checkbox("unlock", self.unlock, "Unlock it first").on_click(cx.listener(
            |this, _: &ClickEvent, _, cx| {
                this.unlock = !this.unlock;
                cx.notify();
            },
        ));
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(line(format!("Folder: {}", tilde(&check.path))))
            .when(!check.branch.is_empty(), |d| d.child(line(format!("Branch {} stays.", check.branch))))
            .child(if check.changed {
                warning("Changes not committed: removing the worktree deletes them.")
            } else {
                line("No changes left uncommitted.")
            })
            .when(check.changed, |d| d.child(force))
            .when_some(check.lock, |d, lock| d.child(warning(lock.text())).child(unlock))
            .when(!check.roles.is_empty(), |d| {
                d.child(line("The worktrees of its flow's roles go with it:"))
                    .children(check.roles.iter().map(|r| bullet(r, colours::muted())))
            })
            .when(check.teardown, |d| d.child(line("The project's teardown runs first.")))
            .child(ui::dim("Its tabs stop, agents included."))
    }
}

impl Checked for RemoveWorktree {
    fn check(&mut self) -> &mut Check {
        &mut self.check
    }
}

impl Focusable for RemoveWorktree {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for RemoveWorktree {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match &self.check {
            Check::Loading => div().child(ui::dim("Checking what removing it would lose…")),
            Check::Failed(e) => div().child(warning(e.clone())),
            Check::Ready(v) => self.report(WorktreeCheck::of(v), cx),
        };
        let ready = self.ready();
        keyed(&self.focus, Self::run, cx).child(frame(
            format!("Remove the worktree {}?", self.name),
            body,
            buttons([ui::danger_button("remove", "Remove worktree")
                .when(!ready, |d| d.opacity(0.4).cursor_default())
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.run(cx)))]),
        ))
    }
}

pub(in crate::sidebar) struct CloseAgent {
    pane: u64,
    check: Check,
    focus: FocusHandle,
}

pub(crate) fn close_agent(pane: u64, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| {
        load_check(args(&["close", "--agent", "--pane", &pane.to_string(), "--check", "--json"]), cx);
        CloseAgent { pane, check: Check::Loading, focus: cx.focus_handle() }
    });
    show(view, window, cx);
}

impl CloseAgent {
    fn run(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.check, Check::Ready(_)) {
            return;
        }
        Store::get(cx).update(cx, |s, cx| {
            s.cmd(args(&["close", "--agent", "--pane", &self.pane.to_string()]));
            s.close_dialog(cx);
        });
    }
}

fn pane_label(p: &Value, cx: &App) -> String {
    let agent = p.get("agent").and_then(Value::as_str).unwrap_or("shell");
    let tab = p.get("tab").and_then(Value::as_u64);
    let store = Store::get(cx);
    let name = store.read(cx).session.as_ref().and_then(|s| {
        s.projects
            .iter()
            .flat_map(|p| &p.workspaces)
            .flat_map(|w| &w.tabs)
            .find(|t| Some(t.id) == tab)
            .map(|t| t.name.clone())
    });
    match name {
        Some(name) => format!("{agent} in “{name}”"),
        None => agent.to_string(),
    }
}

fn pane_list(v: &Value, key: &str, title: &str, colour: gpui::Rgba, cx: &App) -> Vec<Div> {
    let panes: Vec<String> =
        v.get(key).and_then(Value::as_array).into_iter().flatten().map(|p| pane_label(p, cx)).collect();
    if panes.is_empty() {
        return Vec::new();
    }
    std::iter::once(line(title.to_string())).chain(panes.iter().map(|p| bullet(p, colour))).collect()
}

fn close_report(v: &Value, cx: &App) -> (String, Div) {
    let agent = v.get("agent").and_then(Value::as_str).unwrap_or("agent");
    let tabs = v.get("tabs").and_then(Value::as_array).map_or(0, Vec::len);
    let body = div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(line(format!(
            "Closes {tabs} tab{}. Nothing is removed from disk: its worktree and branch stay.",
            if tabs == 1 { "" } else { "s" }
        )))
        .children(pane_list(v, "stops", "Stops:", colours::text(), cx))
        .children(pane_list(v, "keeps", "Keeps running elsewhere:", colours::muted(), cx))
        .children(v.get("warning").and_then(Value::as_str).map(|w| warning(w.to_string())));
    (format!("Close {agent}?"), body)
}

impl Checked for CloseAgent {
    fn check(&mut self) -> &mut Check {
        &mut self.check
    }
}

impl Focusable for CloseAgent {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for CloseAgent {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (title, body) = match &self.check {
            Check::Loading => ("Close agent?".to_string(), div().child(ui::dim("Checking what would stop…"))),
            Check::Failed(e) => ("Close agent?".to_string(), div().child(warning(e.clone()))),
            Check::Ready(v) => close_report(v, cx),
        };
        keyed(&self.focus, Self::run, cx).child(frame(
            title,
            body,
            buttons([ui::danger_button("close-agent", "Close agent")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.run(cx)))]),
        ))
    }
}
