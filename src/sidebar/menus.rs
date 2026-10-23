use gpui::{Context, Pixels, Point, SharedString, Window};

use super::actions::{into_group, ungroup};
use super::workspace::plural;
use super::{Target, cmd, forms};
use crate::app::{Root, tilde};
use crate::context_menu::{Entry, act, danger, item, submenu};
use crate::dialogs;
use crate::model::{Project, PullRequest, Session, Workspace};
use crate::store::ToastKind;

fn rename_item(label: &str, target: Target) -> Entry {
    item(label.to_string(), move |this, window, cx| this.start_rename(target, window, cx))
}

fn fold_item(target: Target, collapsed: bool) -> Entry {
    item(if collapsed { "Expand" } else { "Collapse" }, move |this, _, _| this.set_folded(target, !collapsed))
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

fn path_items(path: &str) -> [Entry; 2] {
    [finder_item(path.to_string()), copy_item("Copy path", path.to_string(), format!("Copied {}", tilde(path)))]
}

fn command_item(label: impl Into<SharedString>, args: Vec<String>) -> Entry {
    item(label, move |this, _, _| this.queue.cmd(args.clone()))
}

fn group_menu(session: &Session, id: u64) -> Vec<Entry> {
    let Some(group) = session.groups.iter().find(|g| g.id == id) else { return Vec::new() };
    let target = Target::Group(id);
    let (name, style_name) = (group.name.clone(), group.name.clone());
    let count = session.projects.iter().filter(|p| p.group == Some(id)).count();
    vec![
        item("Add project…", move |_, _, cx| Root::open_project_into(Some(id), cx)),
        Entry::Separator,
        rename_item("Rename group", target),
        item("Icon and colour…", move |_, window, cx| forms::group_style(id, style_name.clone(), window, cx)),
        fold_item(target, group.collapsed),
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
        item("New workspace…", move |_, window, cx| dialogs::new_workspace(Some(id), window, cx)),
        act("Start agent here", cmd(&["start", "--project", &id.to_string(), "--focus"])),
        Entry::Separator,
        rename_item("Rename project", target),
        submenu("Move to group", move_to_group_items(session, project)),
        fold_item(target, project.collapsed),
        Entry::Separator,
    ];
    entries.extend(path_items(&project.path));
    entries.push(Entry::Separator);
    entries.push(danger("Close project…", move |_, window, cx| {
        dialogs::confirm(
            dialogs::Question::new(
                format!("Close the project {name}?"),
                "Every shell in it stops, agents included. Its folder and worktrees stay on disk.",
                "Close project",
                cmd(&["close", "--project", &id.to_string()]),
            ),
            window,
            cx,
        );
    }));
    entries
}

fn pull_request_menu(pr: &PullRequest) -> Entry {
    let url = pr.url.clone();
    submenu(
        format!("Pull request #{}", pr.number),
        vec![
            Entry::Note(SharedString::from(pr.title.clone())),
            item("Open in browser", move |_, _, cx| cx.open_url(&url)),
            copy_item("Copy link", pr.url.clone(), "Copied the link".into()),
        ],
    )
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
            ),
            window,
            cx,
        );
    })
}

fn workspace_menu(session: &Session, id: u64) -> Vec<Entry> {
    let Some((_, ws)) = session.workspace(id) else { return Vec::new() };
    let ids = id.to_string();
    let mut entries = vec![
        act("New tab here", cmd(&["new-tab", "--workspace", &ids, "--focus"])),
        act("Start agent here", cmd(&["start", "--workspace", &ids, "--focus"])),
    ];
    if let Some(n) = ws.closed_agents.filter(|n| *n > 0) {
        entries.push(command_item(
            format!("Reopen {}", plural(n, "closed agent")),
            cmd(&["agents", "reopen", "--workspace", &ids]),
        ));
    }
    entries.push(Entry::Separator);
    entries.push(rename_item("Rename workspace", Target::Workspace(id)));
    entries.extend(ws.pull_request.as_ref().map(pull_request_menu));
    if !ws.agent_rows().is_empty() {
        entries.push(fold_item(Target::Workspace(id), ws.collapsed));
    }
    entries.push(Entry::Separator);
    entries.extend(path_items(&ws.path));
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
            let verb = if info.collapsed { "expand" } else { "collapse" };
            let label = if info.collapsed { "Expand" } else { "Collapse" };
            entries.push(command_item(label, cmd(&[verb, "--pane", &p])));
        }
        entries.push(Entry::Separator);
        entries.push(danger("Close agent…", move |_, window, cx| forms::close_agent(pane, window, cx)));
        entries
    }
}
