use gpui::{Context, Window};
use serde_json::json;

use crate::app::Root;
use crate::context_menu::{Entry, act, danger, item, submenu};
use crate::dialogs;
use crate::model::{MouseMode, PaneInfo};
use crate::render::moving::{Aim, Place};
use crate::sidebar::cmd;
use crate::store::Busy;

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

fn program_of(info: Option<&PaneInfo>) -> Option<String> {
    info.and_then(|i| i.agent.clone().or(i.program.clone()))
}

fn move_places(pane: u64, target: u64) -> Vec<Entry> {
    PLACES
        .into_iter()
        .map(|(text, place)| item(text, move |this, _, cx| this.move_pane(pane, Aim { target, place }, cx)))
        .collect()
}

fn pass_right_click(pane: u64, (x, y): (u16, u16)) -> Entry {
    item("Send the right click to the program", move |this, _, _| {
        for kind in ["down", "up"] {
            this.queue.input(pane, json!({ "mouse": { "kind": kind, "button": "right", "x": x, "y": y } }));
        }
    })
}

impl Root {
    pub(super) fn close_pane(&mut self, pane: u64, window: &mut Window, cx: &mut Context<Self>) {
        let info = self.session.as_ref().and_then(|s| s.pane(pane));
        let args = cmd(&["close", "--pane", &pane.to_string()]);
        if info.is_none_or(|i| i.agent.is_none()) {
            self.queue.cmd(args);
            return;
        }
        let what = program_of(info).unwrap_or_else(|| "its shell".into());
        let question = dialogs::Question::new(
            "Close this pane?",
            format!("It runs {what}: closing the pane stops it."),
            "Close pane",
            args,
        )
        .busy(Busy::new(format!("close-pane:{pane}"), format!("Closing {what}…")));
        dialogs::confirm(question, window, cx);
    }

    pub(super) fn pane_menu(&self, pane: u64, cell: (u16, u16)) -> Vec<Entry> {
        let p = pane.to_string();
        let info = self.session.as_ref().and_then(|s| s.pane(pane)).cloned();
        let mouse = self.grid.pane(pane).is_some_and(|g| g.modes.mouse != MouseMode::None);
        let mut entries = vec![
            item("Copy", move |this, _, _| this.copy_selection(pane)),
            item("Paste", move |this, _, cx| this.paste(pane, cx)),
            Entry::Separator,
            act("Split right", cmd(&["split", "--pane", &p, "--focus"])),
            act("Split down", cmd(&["split", "--pane", &p, "--down", "--focus"])),
        ];
        entries.extend(self.move_pane_menu(pane));
        if mouse {
            entries.push(pass_right_click(pane, cell));
        }
        let agent = info.as_ref().is_some_and(|i| i.agent.is_some());
        if agent {
            entries.push(Entry::Separator);
            entries.extend(self.agent_menu(pane).into_iter().skip(1));
        }
        entries.push(Entry::Separator);
        entries.push(danger("Close pane", move |this, window, cx| this.close_pane(pane, window, cx)));
        entries
    }

    fn pane_label(&self, id: u64) -> String {
        let program = program_of(self.session.as_ref().and_then(|s| s.pane(id))).unwrap_or_else(|| "shell".into());
        format!("{program} (pane {id})")
    }

    fn move_pane_menu(&self, pane: u64) -> Option<Entry> {
        let others: Vec<u64> = self
            .session
            .as_ref()
            .and_then(|s| s.tab_of_pane(pane))
            .map(|t| t.panes.iter().map(|p| p.id).filter(|&id| id != pane).collect())
            .unwrap_or_default();
        if others.is_empty() {
            return None;
        }
        let targets =
            others.into_iter().map(|target| submenu(self.pane_label(target), move_places(pane, target))).collect();
        Some(submenu("Move pane", targets))
    }
}
