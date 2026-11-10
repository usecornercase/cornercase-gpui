use gpui::{App, Context, Window};
use serde_json::json;

use super::split_args;
use crate::app::Root;
use crate::context_menu::{Entry, act, danger, item, submenu};
use crate::dialogs::{self, CloseWords};
use crate::model::{MouseMode, Session};
use crate::panels::plugins::Spot;
use crate::render::moving::{Aim, Place};
use crate::sidebar::{cmd, path_items};

const PLACES: [(&str, Place); 5] = [
    ("Swap with it", Place::Swap),
    ("Left of it", Place::Left),
    ("Right of it", Place::Right),
    ("Above it", Place::Above),
    ("Below it", Place::Below),
];

pub(super) fn move_pane_args(pane: u64, aim: Aim) -> Vec<String> {
    let (pane, target) = (pane.to_string(), aim.target.to_string());
    cmd(&["move-pane", "--pane", &pane, "--target", &target, "--place", aim.place.arg()])
}

pub(super) fn pane_label(session: Option<&Session>, id: u64) -> String {
    let Some(info) = session.and_then(|s| s.pane(id)) else { return format!("pane {id}") };
    let program = info.running().unwrap_or_else(|| "shell".into());
    format!("{program} (pane {id})")
}

fn move_places<A>(pane: u64, target: u64, act: &A) -> Vec<Entry>
where
    A: Fn(&mut Root, u64, Aim, &mut Context<Root>) + Clone + 'static,
{
    PLACES
        .into_iter()
        .map(|(text, place)| {
            let act = act.clone();
            item(text, move |this, _, cx| act(this, pane, Aim { target, place }, cx))
        })
        .collect()
}

pub(super) fn move_targets(session: Option<&Session>, pane: u64) -> Vec<(u64, String)> {
    let tab = session.and_then(|s| s.tab_of_pane(pane));
    let others = tab.into_iter().flat_map(|t| t.panes.iter().map(|p| p.id)).filter(|&id| id != pane);
    others.map(|id| (id, pane_label(session, id))).collect()
}

pub(super) fn move_menu<A>(pane: u64, targets: Vec<(u64, String)>, act: &A) -> Option<Entry>
where
    A: Fn(&mut Root, u64, Aim, &mut Context<Root>) + Clone + 'static,
{
    if targets.is_empty() {
        return None;
    }
    let targets = targets.into_iter().map(|(target, label)| submenu(label, move_places(pane, target, act)));
    Some(submenu("Move pane", targets.collect()))
}

fn pass_right_click(pane: u64, (x, y): (u16, u16)) -> Entry {
    item("Send the right click to the program", move |this, _, _| {
        for kind in ["down", "up"] {
            this.own.queue.input(pane, json!({ "mouse": { "kind": kind, "button": "right", "x": x, "y": y } }));
        }
    })
}

impl Root {
    pub(super) fn close_pane(&mut self, pane: u64, window: &mut Window, cx: &mut Context<Self>) {
        let info = self.session.as_ref().and_then(|s| s.pane(pane));
        let args = cmd(&["close", "--pane", &pane.to_string()]);
        if info.is_none_or(|i| i.agent.is_none()) {
            self.own.queue.cmd(args);
            return;
        }
        let words = CloseWords::pane(info, None);
        let question = words.question(args).busy(words.busy(format!("close-pane:{pane}")));
        dialogs::confirm(question, window, cx);
    }

    pub(super) fn pane_menu(&self, pane: u64, cell: (u16, u16), cx: &App) -> Vec<Entry> {
        let info = self.session.as_ref().and_then(|s| s.pane(pane)).cloned();
        let mouse = self.own.grid.pane(pane).is_some_and(|g| g.modes.mouse != MouseMode::None);
        let mut entries = vec![
            item("Copy", move |this, _, _| this.copy_selection(pane)),
            item("Paste", move |this, _, cx| this.paste(Some(pane), cx)),
            Entry::Separator,
            act("Split right", split_args(pane, false)),
            act("Split down", split_args(pane, true)),
        ];
        entries.extend(self.move_pane_menu(pane));
        if mouse {
            entries.push(pass_right_click(pane, cell));
        }
        let agent = info.as_ref().is_some_and(|i| i.agent.is_some());
        if agent {
            entries.push(Entry::Separator);
            entries.extend(self.agent_menu(pane, cx).into_iter().skip(1));
        }
        entries.push(Entry::Separator);
        let openers = self.store.read(cx).openers.clone();
        entries.extend(info.and_then(|i| i.path).map(|path| path_items(&path, &openers)).unwrap_or_default());
        entries.push(Entry::Separator);
        entries.push(danger("Close pane", move |this, window, cx| this.close_pane(pane, window, cx)));
        entries.extend(self.plugin_entries(Spot::Pane(pane), cx));
        entries
    }

    fn move_pane_menu(&self, pane: u64) -> Option<Entry> {
        move_menu(pane, move_targets(self.session.as_ref(), pane), &|this: &mut Root, pane, aim, cx| {
            this.move_pane(pane, aim, cx);
        })
    }
}

#[cfg(test)]
mod tests;
