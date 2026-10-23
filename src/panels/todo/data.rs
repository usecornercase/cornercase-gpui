use serde::Deserialize;
use serde_json::Value;

use crate::model::{PaneInfo, Project, Session, Tab, Workspace};
use crate::store::{ToastKind, args};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct Item {
    pub id: u64,
    pub text: String,
    #[serde(default)]
    pub done: bool,
}

pub(crate) fn items(app: Option<&Value>) -> Vec<Item> {
    app.and_then(|app| app.get("todos"))
        .and_then(Value::as_array)
        .map(|todos| todos.iter().filter_map(|t| Item::deserialize(t).ok()).collect())
        .unwrap_or_default()
}

pub(crate) fn undo_number(answer: &Value) -> Option<u64> {
    answer.get("undo").and_then(Value::as_u64)
}

fn move_before(id: &str, before: u64) -> Vec<String> {
    args(&["todo", "move", id, "--before", &before.to_string()])
}

fn move_last(id: &str) -> Vec<String> {
    args(&["todo", "move", id, "--last"])
}

pub(crate) fn step(items: &[Item], id: u64, up: bool) -> Option<Vec<String>> {
    let item = items.iter().find(|i| i.id == id)?;
    let kind: Vec<&Item> = items.iter().filter(|i| i.done == item.done).collect();
    let at = kind.iter().position(|i| i.id == id)?;
    let id = id.to_string();
    if up {
        let before = kind.get(at.checked_sub(1)?)?;
        Some(move_before(&id, before.id))
    } else if at + 1 >= kind.len() {
        None
    } else if let Some(before) = kind.get(at + 2) {
        Some(move_before(&id, before.id))
    } else {
        Some(move_last(&id))
    }
}

pub(crate) fn drop_on(items: &[Item], dragged: u64, target: u64) -> Option<Vec<String>> {
    let from = items.iter().position(|i| i.id == dragged)?;
    let to = items.iter().position(|i| i.id == target)?;
    let id = dragged.to_string();
    if to == from {
        return None;
    }
    if to < from {
        return Some(move_before(&id, target));
    }
    match items.get(to + 1) {
        Some(next) => Some(move_before(&id, next.id)),
        None => Some(move_last(&id)),
    }
}

pub(crate) fn first_and_last(items: &[Item], item: &Item) -> (bool, bool) {
    let mut kind = items.iter().filter(|i| i.done == item.done).map(|i| i.id);
    let first = kind.next();
    let last = kind.next_back().or(first);
    (first == Some(item.id), last == Some(item.id))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Target {
    pub pane: u64,
    pub status: Option<String>,
    pub label: String,
}

fn agent_panes(session: &Session) -> impl Iterator<Item = (&Project, &Workspace, &Tab, &PaneInfo)> {
    session
        .projects
        .iter()
        .flat_map(|p| p.workspaces.iter().map(move |w| (p, w)))
        .flat_map(|(p, w)| w.tabs.iter().map(move |t| (p, w, t)))
        .flat_map(|(p, w, t)| t.panes.iter().map(move |pane| (p, w, t, pane)))
        .filter(|(.., pane)| pane.agent.is_some())
}

fn target(project: &Project, workspace: &Workspace, pane: &PaneInfo) -> Target {
    let agent = pane.agent.clone().unwrap_or_default();
    let label = if project.workspaces.len() > 1 {
        format!("{} › {} · {agent}", project.name, workspace.name)
    } else {
        format!("{} · {agent}", project.name)
    };
    Target { pane: pane.id, status: pane.status.clone(), label }
}

fn disambiguate(found: &mut [(Target, String)], suffix: impl Fn(&Target, &str) -> Option<String>) {
    let labels: Vec<String> = found.iter().map(|(t, _)| t.label.clone()).collect();
    let repeated = |label: &str| labels.iter().filter(|l| *l == label).count() > 1;
    for (target, tab) in found {
        if repeated(&target.label)
            && let Some(suffix) = suffix(target, tab)
        {
            target.label = format!("{} {suffix}", target.label);
        }
    }
}

pub(crate) fn targets(session: &Session) -> Vec<Target> {
    let mut found: Vec<(Target, String)> =
        agent_panes(session).map(|(p, w, t, pane)| (target(p, w, pane), t.name.clone())).collect();
    disambiguate(&mut found, |_, tab| (!tab.is_empty()).then(|| format!("({tab})")));
    disambiguate(&mut found, |target, _| Some(format!("(pane {})", target.pane)));
    found.into_iter().map(|(t, _)| t).collect()
}

pub(crate) fn sent_message(answer: &Value, target: &str) -> (String, ToastKind) {
    if answer.get("unconfirmed").and_then(Value::as_bool) == Some(true) {
        return (format!("sent to {target}, not confirmed: stays to do"), ToastKind::Info);
    }
    if answer.get("checked").and_then(Value::as_bool) == Some(false) {
        return (format!("sent to {target}, but it changed meanwhile: stays to do"), ToastKind::Info);
    }
    (format!("sent to {target}"), ToastKind::Done)
}

pub(super) fn short(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let cut: String = text.chars().take(max.saturating_sub(1)).collect();
    format!("{}…", cut.trim_end())
}
