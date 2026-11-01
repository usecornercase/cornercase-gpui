use gpui::{Context, Pixels, Point, SharedString, Window};

use super::actions::{into_group, ungroup};
use super::workspace::plural;
use super::{Fold, Target, cmd, forms};
use crate::app::{Root, tilde};
use crate::cli;
use crate::context_menu::{Entry, act, act_busy, danger, item, submenu};
use crate::dialogs;
use crate::model::{Project, PullRequest, Session, Workspace};
use crate::panels::PanelKind;
use crate::panels::flows::{FlowStart, start_flow};
use crate::store::{Busy, ToastKind};

fn rename_item(label: &str, target: Target) -> Entry {
    item(label.to_string(), move |this, window, cx| this.start_rename(target, window, cx))
}

fn fold_item(fold: Fold, collapsed: bool) -> Entry {
    item(if collapsed { "Expand" } else { "Collapse" }, move |this, _, _| this.set_folded(fold, !collapsed))
}

fn finder_item(path: String) -> Entry {
    item("Open in Finder", move |_, _, _| {
        let _ = std::process::Command::new("open").arg(&path).spawn();
    })
}

fn copy_item(label: &'static str, text: String, toast: String) -> Entry {
    item(label, move |this, _, cx| {
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.clone()));
        let toast = toast.clone();
        this.store.update(cx, |s, cx| s.toast(toast, ToastKind::Info, cx));
    })
}

fn path_items(path: &str) -> Vec<Entry> {
    let copy = copy_item("Copy path", path.to_string(), format!("Copied {}", tilde(path)));
    if cli::host().is_some() { vec![copy] } else { vec![finder_item(path.to_string()), copy] }
}

fn panel_item(label: &'static str, kind: PanelKind, focus: Option<(&'static str, u64)>) -> Entry {
    item(label, move |this, _, cx| {
        this.store.update(cx, |s, cx| {
            if let Some((what, id)) = focus {
                s.focus(what, id);
            }
            s.open(kind, cx);
        });
    })
}

fn command_item(label: impl Into<SharedString>, args: Vec<String>) -> Entry {
    item(label, move |this, _, _| this.queue.cmd(args.clone()))
}

fn new_workspace_item(project: Option<u64>) -> Entry {
    item("New workspace…", move |_, window, cx| dialogs::new_workspace(project, window, cx))
}

fn start_flow_item(project: Option<u64>) -> Entry {
    item("Start a flow…", move |_, window, cx| start_flow(FlowStart::in_project(project), window, cx))
}

fn group_menu(session: &Session, id: u64) -> Vec<Entry> {
    let Some(group) = session.groups.iter().find(|g| g.id == id) else { return Vec::new() };
    let target = Target::Group(id);
    let (name, style_name) = (group.name.clone(), group.name.clone());
    let count = session.projects.iter().filter(|p| p.group == Some(id)).count();
    vec![
        item("Add project…", move |_, window, cx| Root::open_project_into(Some(id), window, cx)),
        Entry::Separator,
        rename_item("Rename group", target),
        item("Icon and colour…", move |_, window, cx| forms::group_style(id, style_name.clone(), window, cx)),
        fold_item(Fold::Group(id), group.collapsed),
        Entry::Separator,
        danger("Delete group…", move |_, window, cx| forms::delete_group(id, name.clone(), count, window, cx)),
    ]
}

fn move_to_group_items(session: &Session, project: &Project) -> Vec<Entry> {
    let id = project.id;
    let mut groups: Vec<Entry> = session
        .groups
        .iter()
        .filter(|g| Some(g.id) != project.group)
        .map(|g| command_item(format!("{} {}", g.icon.clone().unwrap_or_default(), g.name), into_group(id, g.id)))
        .collect();
    if project.group.is_some() {
        groups.push(command_item("No group", ungroup(id)));
    }
    groups.push(Entry::Separator);
    groups.push(item("New group…", move |_, window, cx| forms::new_group(Some(id), window, cx)));
    groups
}

fn project_menu(session: &Session, id: u64) -> Vec<Entry> {
    let Some(project) = session.projects.iter().find(|p| p.id == id) else { return Vec::new() };
    let target = Target::Project(id);
    let name = project.name.clone();
    let mut entries = vec![
        new_workspace_item(Some(id)),
        start_flow_item(Some(id)),
        act_busy("Start agent here", dialogs::starting(""), cmd(&["start", "--project", &id.to_string(), "--focus"])),
        Entry::Separator,
        rename_item("Rename project", target),
        submenu("Move to group", move_to_group_items(session, project)),
        fold_item(Fold::Project(id), project.collapsed),
        Entry::Separator,
    ];
    entries.extend(path_items(&project.path));
    entries.push(panel_item("Worktrees…", PanelKind::Worktrees, None));
    entries.push(Entry::Separator);
    entries.push(danger("Close project…", move |_, window, cx| {
        dialogs::confirm(
            dialogs::Question::new(
                format!("Close the project {name}?"),
                "Every shell in it stops, agents included. Its folder and worktrees stay on disk.",
                "Close project",
                cmd(&["close", "--project", &id.to_string()]),
            )
            .busy(Busy::new(format!("close-project:{id}"), format!("Closing {name}…"))),
            window,
            cx,
        );
    }));
    entries
}

fn pull_request_item(workspace: u64, pr: &PullRequest) -> Entry {
    item(format!("{}…", pr.heading()), move |_, window, cx| dialogs::pull_request(workspace, window, cx))
}

fn close_workspace_item(ws: &Workspace) -> Entry {
    let (id, name) = (ws.id, ws.name.clone());
    let label = if ws.worktree { "Close its tabs…" } else { "Close workspace…" };
    let message = if ws.worktree {
        "Every tab in it stops, agents included. The worktree stays on disk and listed."
    } else {
        "Every tab in it stops, agents included. Its folder stays on disk."
    };
    danger(label, move |_, window, cx| {
        dialogs::confirm(
            dialogs::Question::new(
                format!("Close {name}?"),
                message,
                "Close",
                cmd(&["close", "--workspace", &id.to_string()]),
            )
            .busy(Busy::new(format!("close-workspace:{id}"), format!("Closing {name}…"))),
            window,
            cx,
        );
    })
}

fn ask_item(label: String, question: impl Fn() -> dialogs::Question + 'static) -> Entry {
    danger(label, move |_, window, cx| dialogs::confirm(question(), window, cx))
}

fn agent_items(ws: &Workspace) -> Vec<Entry> {
    let (ids, name) = (ws.id.to_string(), ws.name.clone());
    let mut entries = Vec::new();
    if !ws.agent_rows().is_empty() {
        let (ws_id, name) = (ws.id, name.clone());
        entries.push(danger("Close its agents…", move |_, _, cx| forms::close_agents(ws_id, name.clone(), cx)));
    }
    if let Some(n) = ws.closed_agents.filter(|n| *n > 0) {
        let reopening = Busy::new(format!("reopen:{ids}"), format!("Reopening the agents of {name}…"));
        let reopen = cmd(&["agents", "reopen", "--workspace", &ids]);
        entries.push(item(format!("Reopen {}", plural(n, "closed agent")), move |this, _, cx| {
            let (busy, args) = (reopening.clone(), reopen.clone());
            this.store.update(cx, |s, cx| s.run_aside(busy, args, cx));
        }));
        entries.push(ask_item(format!("Forget {}…", plural(n, "closed agent")), move || {
            dialogs::Question::new(
                format!("Forget the closed agents of {name}?"),
                "They can no longer be reopened from here; their conversations stay with each agent.",
                "Forget",
                cmd(&["agents", "forget", "--workspace", &ids, "--yes"]),
            )
            .busy(Busy::new(format!("forget:{ids}"), format!("Forgetting the closed agents of {name}…")))
        }));
    }
    entries
}

fn workspace_menu(session: &Session, id: u64) -> Vec<Entry> {
    let Some((_, ws)) = session.workspace(id) else { return Vec::new() };
    let ids = id.to_string();
    let mut entries = vec![
        act("New tab here", cmd(&["new-tab", "--workspace", &ids, "--focus"])),
        act_busy("Start agent here", dialogs::starting(""), cmd(&["start", "--workspace", &ids, "--focus"])),
    ];
    entries.extend(agent_items(ws));
    entries.push(Entry::Separator);
    entries.push(rename_item("Rename workspace", Target::Workspace(id)));
    entries.extend(ws.pull_request.as_ref().map(|pr| pull_request_item(id, pr)));
    if ws.runs_flow() {
        entries.push(panel_item("View flow", PanelKind::Flow, Some(("workspace", id))));
    }
    if !ws.agent_rows().is_empty() {
        entries.push(fold_item(Fold::Workspace(id), ws.collapsed));
    }
    entries.push(Entry::Separator);
    entries.extend(path_items(&ws.path));
    entries.push(panel_item("Ports…", PanelKind::Ports, Some(("workspace", id))));
    entries.push(Entry::Separator);
    if ws.worktree {
        let name = ws.name.clone();
        entries.push(danger("Remove worktree…", move |_, window, cx| {
            forms::remove_worktree(id, name.clone(), window, cx);
        }));
    }
    if !ws.tabs.is_empty() || !ws.worktree {
        entries.push(close_workspace_item(ws));
    }
    entries
}

impl Root {
    pub(crate) fn open_target_menu(
        &mut self,
        target: Target,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let entries = self.menu_for(target);
        self.open_menu(at, entries, window, cx);
    }

    pub(super) fn open_new_menu(
        &mut self,
        project: Option<u64>,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_menu(at, vec![new_workspace_item(project), start_flow_item(project)], window, cx);
    }

    pub(super) fn open_agent_menu(
        &mut self,
        pane: u64,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let entries = self.agent_menu(pane);
        self.open_menu(at, entries, window, cx);
    }

    fn menu_for(&self, target: Target) -> Vec<Entry> {
        let Some(session) = &self.session else { return Vec::new() };
        match target {
            Target::Group(id) => group_menu(session, id),
            Target::Project(id) => project_menu(session, id),
            Target::Workspace(id) => workspace_menu(session, id),
            Target::Tab(id) => self.tab_menu(id),
        }
    }

    fn agent_has_children(&self, pane: u64) -> bool {
        self.session
            .as_ref()
            .and_then(|s| {
                s.projects
                    .iter()
                    .flat_map(|p| &p.workspaces)
                    .find_map(|w| w.agent_rows().into_iter().find(|r| r.pane.id == pane))
            })
            .is_some_and(|r| r.children > 0)
    }

    pub(crate) fn agent_menu(&self, pane: u64) -> Vec<Entry> {
        let Some(info) = self.session.as_ref().and_then(|s| s.pane(pane)) else { return Vec::new() };
        let p = pane.to_string();
        let mut entries = vec![act("Show", cmd(&["focus", "--pane", &p]))];
        if info.status.as_deref() == Some("pending") {
            entries.push(act("Mark as reviewed", cmd(&["mark", "--pane", &p, "--reviewed"])));
        } else {
            entries.push(act("Mark as pending review", cmd(&["mark", "--pane", &p])));
        }
        if self.agent_has_children(pane) {
            entries.push(fold_item(Fold::Pane(pane), info.collapsed));
        }
        entries.push(Entry::Separator);
        entries.push(danger("Close agent…", move |_, window, cx| forms::close_agent(pane, window, cx)));
        entries
    }
}
