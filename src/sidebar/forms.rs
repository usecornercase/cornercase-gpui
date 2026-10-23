//! The sidebar's forms: a name for a new group, a group's icon and colour, deleting a group,
//! removing a worktree (with what `close --check` says would be lost) and closing an agent
//! (with what `close --agent --check` says would stop).

use gpui::{
    App, AppContext as _, ClickEvent, Context, Entity, FocusHandle, Focusable, InteractiveElement as _, IntoElement,
    KeyDownEvent, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _, px,
};
use serde_json::Value;

use crate::app::tilde;
use crate::dialogs::frame;
use crate::store::{self, Store, args};
use crate::theme::colours;
use crate::ui::{self, TextField, TextFieldEvent};

fn close(cx: &mut App) {
    Store::get(cx).update(cx, |s, cx| s.close_dialog(cx));
}

fn show(view: gpui::AnyView, focus: FocusHandle, window: &mut Window, cx: &mut App) {
    Store::get(cx).update(cx, |s, cx| s.show_dialog(view, cx));
    window.focus(&focus);
}

/// A palette index as a colour, for group icons.
pub fn palette_colour(index: u8) -> gpui::Rgba {
    let [r, g, b] = crate::theme::palette(index);
    gpui::rgb((u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b))
}

/// What a `--check` command printed: loading, its JSON, or why it failed.
enum Check {
    Loading,
    Ready(Value),
    Failed(String),
}

impl Check {
    fn from(result: anyhow::Result<Value>) -> Self {
        match result {
            Ok(v) => Check::Ready(v),
            Err(e) => Check::Failed(e.to_string().lines().next().unwrap_or_default().to_string()),
        }
    }
}

fn line(text: impl Into<SharedString>) -> gpui::Div {
    div().text_color(colours::muted()).child(text.into())
}

fn warning(text: impl Into<SharedString>) -> gpui::Div {
    div().text_color(colours::failed()).child(text.into())
}

// ---------------------------------------------------------------------------------------------

/// `cornercase group new NAME`, then optionally moves a project into it.
pub struct NewGroup {
    name: Entity<TextField>,
    project: Option<u64>,
}

pub fn new_group(project: Option<u64>, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| {
        let name = cx.new(|cx| TextField::new("work", cx));
        cx.subscribe_in(&name, window, |this: &mut NewGroup, _, event, _, cx| match event {
            TextFieldEvent::Submit => this.submit(cx),
            TextFieldEvent::Cancel => close(cx),
            TextFieldEvent::Changed => cx.notify(),
        })
        .detach();
        NewGroup { name, project }
    });
    let focus = view.read(cx).name.read(cx).focus_handle(cx);
    show(view.into(), focus, window, cx);
}

impl NewGroup {
    fn submit(&mut self, cx: &mut Context<Self>) {
        let name = self.name.read(cx).text().trim().to_string();
        if name.is_empty() {
            return;
        }
        let project = self.project;
        store::run_json(cx, args(&["group", "new", &name, "--json"]), move |_, result, cx| {
            let store = Store::get(cx);
            match result.map(|v| v.get("group").and_then(Value::as_u64)) {
                Ok(Some(group)) => {
                    if let Some(project) = project {
                        let a = args(&["move", "--project", &project.to_string(), "--into", &group.to_string()]);
                        store.read(cx).cmd(a);
                    }
                    store.update(cx, |s, cx| s.close_dialog(cx));
                }
                Ok(None) => store.update(cx, |s, cx| s.toast("cornercase group new printed no id", store::ToastKind::Error, cx)),
                Err(e) => store.update(cx, |s, cx| s.toast(e.to_string(), store::ToastKind::Error, cx)),
            }
        });
    }
}

impl Render for NewGroup {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.name.read(cx).focus_handle(cx).is_focused(window);
        frame(
            "New group",
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(ui::field("Name", ui::input_box(self.name.clone(), focused)))
                .child(ui::dim("It takes the next icon and colour; change them from its menu.")),
            div()
                .flex()
                .gap(px(8.))
                .child(ui::button("cancel", "Cancel").on_click(|_: &ClickEvent, _, cx| close(cx)))
                .child(
                    ui::primary_button("create", "Create").on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.submit(cx))),
                ),
        )
    }
}

// ---------------------------------------------------------------------------------------------

/// A group's icon and colour: `group style ID --json` lists the choices, a click applies at once.
pub struct GroupStyle {
    group: u64,
    name: String,
    check: Check,
    focus: FocusHandle,
}

pub fn group_style(group: u64, name: String, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| {
        store::run_json(cx, args(&["group", "style", &group.to_string(), "--json"]), |this: &mut GroupStyle, r, cx| {
            this.check = Check::from(r);
            cx.notify();
        });
        GroupStyle { group, name, check: Check::Loading, focus: cx.focus_handle() }
    });
    let focus = view.read(cx).focus.clone();
    show(view.into(), focus, window, cx);
}

impl GroupStyle {
    fn set(&mut self, key: &str, value: Value, cx: &mut Context<Self>) {
        let text = match &value {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        let flag = if key == "icon" { "--icon" } else { "--colour" };
        Store::get(cx).read(cx).cmd(args(&["group", "style", &self.group.to_string(), flag, &text]));
        if let Check::Ready(v) = &mut self.check
            && let Some(obj) = v.as_object_mut()
        {
            obj.insert(key.to_string(), value);
        }
        cx.notify();
    }
}

impl Render for GroupStyle {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match &self.check {
            Check::Loading => ui::dim("Loading…"),
            Check::Failed(e) => warning(e.clone()),
            Check::Ready(v) => {
                let icon = v.get("icon").and_then(Value::as_str).unwrap_or_default().to_string();
                let colour = v.get("colour").and_then(Value::as_u64).unwrap_or(4) as u8;
                let icons: Vec<String> =
                    v.get("icons").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(String::from).collect();
                let colours_: Vec<u8> =
                    v.get("colours").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_u64).map(|c| c as u8).collect();
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(div().text_size(px(18.)).text_color(palette_colour(colour)).child(icon.clone()))
                            .child(ui::title(self.name.clone())),
                    )
                    .child(ui::field(
                        "Icon",
                        div().flex().flex_wrap().gap(px(4.)).children(icons.into_iter().enumerate().map(|(i, glyph)| {
                            let selected = glyph == icon;
                            div()
                                .id(SharedString::from(format!("icon-{i}")))
                                .size(px(28.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(5.))
                                .border_1()
                                .border_color(if selected { colours::accent() } else { colours::border() })
                                .when(selected, |d| d.bg(colours::selected()))
                                .text_color(palette_colour(colour))
                                .cursor_pointer()
                                .hover(|s| s.bg(colours::hover()))
                                .child(glyph.clone())
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.set("icon", Value::String(glyph.clone()), cx)
                                }))
                        })),
                    ))
                    .child(ui::field(
                        "Colour",
                        div().flex().flex_wrap().gap(px(6.)).children(colours_.into_iter().map(|c| {
                            let selected = c == colour;
                            div()
                                .id(SharedString::from(format!("colour-{c}")))
                                .size(px(22.))
                                .rounded_full()
                                .border_2()
                                .border_color(if selected { colours::text() } else { colours::panel() })
                                .bg(palette_colour(c))
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set("colour", Value::from(c), cx)))
                        })),
                    ))
            }
        };
        div()
            .track_focus(&self.focus)
            .on_key_down(|e: &KeyDownEvent, _, cx| {
                if matches!(e.keystroke.key.as_str(), "escape" | "enter") {
                    close(cx);
                }
            })
            .child(frame(
                "Icon and colour",
                body,
                ui::primary_button("done", "Done").on_click(|_: &ClickEvent, _, cx| close(cx)),
            ))
    }
}

// ---------------------------------------------------------------------------------------------

/// Deleting a group that holds projects: keep them (ungrouped) or close them too.
pub struct DeleteGroup {
    group: u64,
    name: String,
    projects: usize,
    focus: FocusHandle,
}

pub fn delete_group(group: u64, name: String, projects: usize, window: &mut Window, cx: &mut App) {
    if projects == 0 {
        crate::dialogs::confirm(
            format!("Delete the group {name}?"),
            "It holds no projects.",
            "Delete group",
            args(&["group", "delete", &group.to_string()]),
            false,
            window,
            cx,
        );
        return;
    }
    let view = cx.new(|cx| DeleteGroup { group, name, projects, focus: cx.focus_handle() });
    let focus = view.read(cx).focus.clone();
    show(view.into(), focus, window, cx);
}

impl DeleteGroup {
    fn run(&self, close_projects: bool, cx: &mut App) {
        let mut a = args(&["group", "delete", &self.group.to_string()]);
        if close_projects {
            a.push("--close-projects".into());
        }
        Store::get(cx).update(cx, |s, cx| {
            s.cmd(a);
            s.close_dialog(cx);
        });
    }
}

impl Render for DeleteGroup {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let n = self.projects;
        let plural = if n == 1 { "project" } else { "projects" };
        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| match e.keystroke.key.as_str() {
                "enter" => this.run(false, cx),
                "escape" => close(cx),
                _ => {}
            }))
            .child(frame(
                format!("Delete the group {}?", self.name),
                div().flex().flex_col().gap(px(6.)).child(line(format!(
                    "It holds {n} {plural}. Keep them outside any group, or close them too: closing stops every \
                     shell in them, agents included. Folders and worktrees stay on disk."
                ))),
                div()
                    .flex()
                    .gap(px(8.))
                    .child(ui::button("cancel", "Cancel").on_click(|_: &ClickEvent, _, cx| close(cx)))
                    .child(
                        ui::danger_button("close-projects", format!("Close {plural}"))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.run(true, cx))),
                    )
                    .child(
                        ui::primary_button("keep", format!("Keep {plural}"))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.run(false, cx))),
                    ),
            ))
    }
}

// ---------------------------------------------------------------------------------------------

/// Removing a worktree: what `close --workspace ID --remove-worktree --check --json` says would
/// be lost (changes not committed, a lock, a flow's role worktrees, the teardown), then the removal.
pub struct RemoveWorktree {
    workspace: u64,
    name: String,
    check: Check,
    force: bool,
    unlock: bool,
    focus: FocusHandle,
}

pub fn remove_worktree(workspace: u64, name: String, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| {
        let a = args(&["close", "--workspace", &workspace.to_string(), "--remove-worktree", "--check", "--json"]);
        store::run_json(cx, a, |this: &mut RemoveWorktree, r, cx| {
            this.check = Check::from(r);
            cx.notify();
        });
        RemoveWorktree { workspace, name, check: Check::Loading, force: false, unlock: false, focus: cx.focus_handle() }
    });
    let focus = view.read(cx).focus.clone();
    show(view.into(), focus, window, cx);
}

impl RemoveWorktree {
    /// Whether the removal can go: the check is in, and any changes and lock are acknowledged.
    fn ready(&self) -> bool {
        match &self.check {
            Check::Ready(v) => {
                let changed = v.get("changed").and_then(Value::as_bool).unwrap_or(false);
                let locked = v.get("lock").is_some_and(|l| !l.is_null());
                (!changed || self.force) && (!locked || self.unlock)
            }
            _ => false,
        }
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
}

impl Render for RemoveWorktree {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match &self.check {
            Check::Loading => div().child(ui::dim("Checking what removing it would lose…")),
            Check::Failed(e) => div().child(warning(e.clone())),
            Check::Ready(v) => {
                let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
                let changed = v.get("changed").and_then(Value::as_bool).unwrap_or(false);
                let lock = v.get("lock").filter(|l| !l.is_null());
                let roles: Vec<String> = v
                    .get("roles")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .map(|r| {
                        let get = |k: &str| r.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
                        format!("{} · {}", get("role"), get("branch"))
                    })
                    .collect();
                let teardown = v.get("teardown").and_then(Value::as_bool).unwrap_or(false);
                let mut body = div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(line(format!("Folder: {}", tilde(&s("path")))))
                    .when(!s("branch").is_empty(), |d| d.child(line(format!("Branch {} stays.", s("branch")))))
                    .child(if changed {
                        warning("Changes not committed: removing the worktree deletes them.")
                    } else {
                        line("No changes left uncommitted.")
                    });
                if changed {
                    body = body.child(
                        ui::checkbox("force", self.force, "Delete them anyway").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.force = !this.force;
                            cx.notify();
                        })),
                    );
                }
                if let Some(lock) = lock {
                    let reason = lock.get("reason").and_then(Value::as_str).unwrap_or("no reason given");
                    let running = match lock.get("running").and_then(Value::as_bool) {
                        Some(true) => ", and the process holding it still runs",
                        Some(false) => ", by a process that no longer runs",
                        None => "",
                    };
                    body = body.child(warning(format!("Locked: {reason}{running}."))).child(
                        ui::checkbox("unlock", self.unlock, "Unlock it first").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.unlock = !this.unlock;
                            cx.notify();
                        })),
                    );
                }
                if !roles.is_empty() {
                    body = body.child(line("The worktrees of its flow's roles go with it:")).children(
                        roles.into_iter().map(|r| div().pl(px(12.)).text_color(colours::muted()).child(format!("• {r}"))),
                    );
                }
                if teardown {
                    body = body.child(line("The project's teardown runs first."));
                }
                body.child(ui::dim("Its tabs stop, agents included."))
            }
        };
        let ready = self.ready();
        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| match e.keystroke.key.as_str() {
                "enter" => this.run(cx),
                "escape" => close(cx),
                _ => {}
            }))
            .child(frame(
                format!("Remove the worktree {}?", self.name),
                body,
                div()
                    .flex()
                    .gap(px(8.))
                    .child(ui::button("cancel", "Cancel").on_click(|_: &ClickEvent, _, cx| close(cx)))
                    .child(
                        ui::danger_button("remove", "Remove worktree")
                            .when(!ready, |d| d.opacity(0.4).cursor_default())
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.run(cx))),
                    ),
            ))
    }
}

// ---------------------------------------------------------------------------------------------

/// Closing an agent: its tab and the tabs of the agents it started in that workspace, as
/// `close --agent --pane ID --check --json` lists them. Nothing on disk goes.
pub struct CloseAgent {
    pane: u64,
    check: Check,
    focus: FocusHandle,
}

pub fn close_agent(pane: u64, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| {
        let a = args(&["close", "--agent", "--pane", &pane.to_string(), "--check", "--json"]);
        store::run_json(cx, a, |this: &mut CloseAgent, r, cx| {
            this.check = Check::from(r);
            cx.notify();
        });
        CloseAgent { pane, check: Check::Loading, focus: cx.focus_handle() }
    });
    let focus = view.read(cx).focus.clone();
    show(view.into(), focus, window, cx);
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

/// `claude in “fix login”` for a pane the check lists, from the session.
fn pane_label(p: &Value, cx: &App) -> String {
    let agent = p.get("agent").and_then(Value::as_str).unwrap_or("shell");
    let tab = p.get("tab").and_then(Value::as_u64);
    let store = Store::get(cx);
    let name = store.read(cx).session.as_ref().and_then(|s| {
        s.projects.iter().flat_map(|p| &p.workspaces).flat_map(|w| &w.tabs).find(|t| Some(t.id) == tab).map(|t| t.name.clone())
    });
    match name {
        Some(name) => format!("{agent} in “{name}”"),
        None => agent.to_string(),
    }
}

impl Render for CloseAgent {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (title, body) = match &self.check {
            Check::Loading => ("Close agent?".to_string(), div().child(ui::dim("Checking what would stop…"))),
            Check::Failed(e) => ("Close agent?".to_string(), div().child(warning(e.clone()))),
            Check::Ready(v) => {
                let agent = v.get("agent").and_then(Value::as_str).unwrap_or("agent");
                let list = |k: &str| -> Vec<String> {
                    v.get(k).and_then(Value::as_array).into_iter().flatten().map(|p| pane_label(p, cx)).collect()
                };
                let (stops, keeps) = (list("stops"), list("keeps"));
                let tabs = v.get("tabs").and_then(Value::as_array).map_or(0, Vec::len);
                let mut body = div().flex().flex_col().gap(px(6.)).child(line(format!(
                    "Closes {tabs} tab{}. Nothing is removed from disk: its worktree and branch stay.",
                    if tabs == 1 { "" } else { "s" }
                )));
                if !stops.is_empty() {
                    body = body
                        .child(line("Stops:"))
                        .children(stops.into_iter().map(|s| div().pl(px(12.)).text_color(colours::text()).child(format!("• {s}"))));
                }
                if !keeps.is_empty() {
                    body = body
                        .child(line("Keeps running elsewhere:"))
                        .children(keeps.into_iter().map(|s| div().pl(px(12.)).text_color(colours::muted()).child(format!("• {s}"))));
                }
                if let Some(w) = v.get("warning").and_then(Value::as_str) {
                    body = body.child(warning(w.to_string()));
                }
                (format!("Close {agent}?"), body)
            }
        };
        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| match e.keystroke.key.as_str() {
                "enter" => this.run(cx),
                "escape" => close(cx),
                _ => {}
            }))
            .child(frame(
                title,
                body,
                div()
                    .flex()
                    .gap(px(8.))
                    .child(ui::button("cancel", "Cancel").on_click(|_: &ClickEvent, _, cx| close(cx)))
                    .child(
                        ui::danger_button("close-agent", "Close agent")
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.run(cx))),
                    ),
            ))
    }
}
