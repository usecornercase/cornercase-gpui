use gpui::{Context, Pixels, Point, Window};
use serde_json::json;

use super::{PanePress, surface_cell};
use crate::app::Root;
use crate::context_menu::{Entry, act, danger, item, submenu};
use crate::dialogs;
use crate::model::{Area, MouseMode, PaneInfo};
use crate::sidebar::cmd;

const PLACES: [(&str, &str); 5] = [
    ("Swap with it", "swap"),
    ("Left of it", "left"),
    ("Right of it", "right"),
    ("Above it", "above"),
    ("Below it", "below"),
];

pub(super) fn pane_place(target: Area, x: u16, y: u16) -> &'static str {
    let fx = (f32::from(x.saturating_sub(target.x)) + 0.5) / f32::from(target.cols.max(1));
    let fy = (f32::from(y.saturating_sub(target.y)) + 0.5) / f32::from(target.rows.max(1));
    let middle = |f: f32| (1. / 3. ..=2. / 3.).contains(&f);
    if middle(fx) && middle(fy) {
        return "swap";
    }
    [("left", fx), ("right", 1. - fx), ("above", fy), ("below", 1. - fy)]
        .into_iter()
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map_or("swap", |(place, _)| place)
}

fn move_pane_args(pane: u64, target: u64, place: &str) -> Vec<String> {
    cmd(&["move-pane", "--pane", &pane.to_string(), "--target", &target.to_string(), "--place", place])
}

fn program_of(info: Option<&PaneInfo>) -> Option<String> {
    info.and_then(|i| i.agent.clone().or(i.program.clone()))
}

fn move_places(pane: u64, target: u64) -> Vec<Entry> {
    PLACES
        .into_iter()
        .map(|(text, place)| {
            let args = move_pane_args(pane, target, place);
            item(text, move |this, _, _| this.queue.cmd(args.clone()))
        })
        .collect()
}

fn pass_right_click(pane: u64, (x, y): (u16, u16)) -> Entry {
    item("Send the right click to the program", move |this, _, _| {
        for kind in ["down", "up"] {
            this.queue.input(pane, json!({ "mouse": { "kind": kind, "button": "right", "x": x, "y": y } }));
        }
    })
}

fn close_pane(pane: u64, info: Option<&PaneInfo>, agent: bool) -> Entry {
    let what = program_of(info).unwrap_or_else(|| "its shell".into());
    danger("Close pane", move |this, window, cx| {
        let args = cmd(&["close", "--pane", &pane.to_string()]);
        if !agent {
            this.queue.cmd(args);
            return;
        }
        dialogs::confirm(
            dialogs::Question::new(
                "Close this pane?",
                format!("It runs {what}: closing the pane stops it."),
                "Close pane",
                args,
            ),
            window,
            cx,
        );
    })
}

impl Root {
    fn pane_menu(&self, pane: u64, cell: (u16, u16)) -> Vec<Entry> {
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
        entries.push(close_pane(pane, info.as_ref(), agent));
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

    pub(super) fn pane_release(
        &mut self,
        press: &PanePress,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((x, y)) = self.cell(position).and_then(surface_cell) else { return };
        let Some((target, rect)) = self.grid.pane_at(x, y).map(|t| (t.pane, t.rect)) else { return };
        if target == press.pane {
            let entries = self.pane_menu(press.pane, press.cell);
            self.open_menu(position, entries, window, cx);
            return;
        }
        self.queue.cmd(move_pane_args(press.pane, target, pane_place(rect, x, y)));
    }
}
