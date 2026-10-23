//! The terminal: drawing the surface, and the keys, text, mouse and wheel that go to its panes.
//! The left button selects (a double click a word, a triple click a line, copied as the TUI
//! does) unless the program reads the mouse; the right button is cornercase's in every pane: a
//! click opens the pane's menu, a drag onto another pane moves it there (`move-pane`).

use std::ops::Range;

use gpui::{
    Bounds, Context, ElementInputHandler, EntityInputHandler, InteractiveElement, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, ScrollWheelEvent,
    Styled, UTF16Selection, Window, canvas, div, point, prelude::FluentBuilder as _,
    px, size,
};
use serde_json::json;

use crate::app::{Conn, Drag, FONT_FAMILY, Msg, PADDING, Root};
use crate::context_menu::{Entry, act, danger, item, submenu};
use crate::dialogs;
use crate::keys;
use crate::model::{Area, MouseMode, PaneGrid};
use crate::panels::PanelKind;
use crate::render::{self, Metrics};
use crate::sidebar::cmd;
use crate::theme::colours;

/// A right press on a pane, kept until the release decides between its menu and a move.
pub struct PanePress {
    pub pane: u64,
    /// The press's cell in the pane, for passing the right click on to the program.
    pub cell: (u16, u16),
}

/// Where a pane dropped at a cell of `target` goes, as `move-pane --place` says it: the middle
/// third both ways swaps the two, elsewhere the nearest edge takes that side.
pub fn pane_place(target: Area, x: u16, y: u16) -> &'static str {
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

/// The class of a cell for word selection: blank, part of a word (paths and links included),
/// or other punctuation.
fn class(cell: &str) -> u8 {
    match cell.chars().next() {
        None | Some(' ') => 0,
        Some(c) if c.is_alphanumeric() || "_-./~:@%+=#?&".contains(c) => 1,
        Some(_) => 2,
    }
}

/// The columns of the word under `x` in a line of cells: a run of the same class. The empty cell
/// after a wide character belongs to it.
pub fn word_at(cells: &[&str], x: usize) -> (usize, usize) {
    if x >= cells.len() {
        return (x, x);
    }
    let classes: Vec<u8> = cells
        .iter()
        .enumerate()
        .map(|(i, c)| if c.is_empty() && i > 0 && !cells[i - 1].is_empty() { class(cells[i - 1]) } else { class(c) })
        .collect();
    let at = classes[x];
    let start = (0..x).rev().take_while(|&i| classes[i] == at).last().unwrap_or(x);
    let end = (x + 1..cells.len()).take_while(|&i| classes[i] == at).last().unwrap_or(x);
    (start, end)
}

impl Root {
    // -----------------------------------------------------------------------------------------
    // Keyboard.

    pub(crate) fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let ks = &event.keystroke;
        let m = ks.modifiers;
        // Keys go to the terminal only while it has the focus, not while a field or panel has it.
        let terminal = self.focus.is_focused(window);
        if m.platform {
            if self.shortcut(ks.key.as_str(), m.shift, terminal, window, cx) {
                cx.stop_propagation();
            }
            return;
        }
        if !terminal {
            return;
        }
        if let Some(event) = keys::key_event(ks) {
            self.input(event);
            cx.stop_propagation();
        }
    }

    /// ⌘ shortcuts; true when taken.
    pub(crate) fn shortcut(&mut self, key: &str, shift: bool, terminal: bool, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let store = self.store.clone();
        let panel = |kind: PanelKind, cx: &mut Context<Self>| store.update(cx, |s, cx| s.open(kind, cx));
        let dock = |kind: PanelKind, cx: &mut Context<Self>| self.store.update(cx, |s, cx| s.toggle_dock(kind, cx));
        match key {
            "c" if shift => dock(PanelKind::Changes, cx),
            "e" if shift => dock(PanelKind::Files, cx),
            "l" if shift => dock(PanelKind::Todo, cx),
            "," => panel(PanelKind::Settings, cx),
            "i" => panel(PanelKind::Issues, cx),
            "u" => panel(PanelKind::Usage, cx),
            "f" if shift => panel(PanelKind::Search, cx),
            "c" if terminal => {
                if let Some(pane) = self.active_pane() {
                    self.copy_selection(pane);
                }
            }
            "v" if terminal => {
                if let Some(pane) = self.active_pane() {
                    self.paste(pane, cx);
                }
            }
            "p" if !shift => self.focus_filter(window, cx),
            "t" => self.act(vec!["new-tab".into(), "--focus".into()]),
            "d" => self.split(shift),
            "o" => self.open_project(window, cx),
            "n" => dialogs::new_workspace(None, window, cx),
            "g" => dialogs::start_agent(window, cx),
            "=" | "+" => self.zoom(1., cx),
            "-" => self.zoom(-1., cx),
            "0" => {
                self.font_size = 13.;
                self.metrics = None;
                cx.notify();
            }
            "[" | "]" if shift => self.cycle_tab(if key == "]" { 1 } else { -1 }),
            "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" => {
                let n: usize = key.parse().unwrap_or(1);
                if let Some(tab) = self.shown_workspace().and_then(|(_, w)| w.tabs.get(n - 1)).map(|t| t.id) {
                    self.focus_item("tab", tab);
                }
            }
            _ => return false,
        }
        true
    }

    /// Copies a pane's selection to the clipboard, with a toast.
    pub(crate) fn copy_selection(&self, pane: u64) {
        let tx = self.tx.clone();
        let args = ["selection", "--pane", &pane.to_string(), "--copy", "--json"].map(String::from);
        self.queue.cmd_then(args.to_vec(), move |r| {
            let text = r.ok().and_then(|out| {
                serde_json::from_str::<serde_json::Value>(&out)
                    .ok()
                    .and_then(|v| v.get("text").and_then(|t| t.as_str()).map(String::from))
            });
            if let Some(text) = text.filter(|t| !t.is_empty()) {
                let _ = tx.unbounded_send(Msg::Copied(text));
            }
        });
    }

    pub(crate) fn paste(&self, pane: u64, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.queue.input(pane, json!({ "paste": text }));
        }
    }

    /// Selects from one cell to another of a pane, then copies it.
    fn select(&self, pane: u64, from: (u16, u16), to: (u16, u16)) {
        self.queue.cmd(vec![
            "selection".into(),
            "--pane".into(),
            pane.to_string(),
            "--from".into(),
            format!("{},{}", from.0, from.1),
            "--to".into(),
            format!("{},{}", to.0, to.1),
        ]);
        self.copy_selection(pane);
    }

    /// A double click selects the word under it, a triple click the line.
    fn select_by_clicks(&self, pane: &PaneGrid, x: u16, y: u16, clicks: usize) {
        let cells: Vec<&str> = pane
            .lines
            .get(y as usize)
            .map(|line| line.iter().flat_map(|run| run.cells.iter().map(String::as_str)).collect())
            .unwrap_or_default();
        if clicks >= 3 {
            self.select(pane.pane, (0, y), (pane.size.cols.saturating_sub(1), y));
        } else if (x as usize) < cells.len() && class(cells[x as usize]) != 0 {
            let (start, end) = word_at(&cells, x as usize);
            self.select(pane.pane, (start as u16, y), (end as u16, y));
        }
    }

    /// The menu of a pane: copy and paste, splits, moving it, review marks, closing.
    fn pane_menu(&self, pane: u64, cell: (u16, u16)) -> Vec<Entry> {
        let p = pane.to_string();
        let info = self.session.as_ref().and_then(|s| s.pane(pane)).cloned();
        let mouse = self.grid.pane(pane).is_some_and(|g| g.modes.mouse != MouseMode::None);
        let label = |id: u64| {
            let info = self.session.as_ref().and_then(|s| s.pane(id));
            let program = info.and_then(|i| i.agent.clone().or(i.program.clone())).unwrap_or_else(|| "shell".into());
            format!("{program} (pane {id})")
        };
        let mut entries = vec![
            item("Copy", move |this, _, _| this.copy_selection(pane)),
            item("Paste", move |this, _, cx| this.paste(pane, cx)),
            Entry::Separator,
            act("Split right", cmd(&["split", "--pane", &p, "--focus"])),
            act("Split down", cmd(&["split", "--pane", &p, "--down", "--focus"])),
        ];
        let others: Vec<u64> = self
            .session
            .as_ref()
            .and_then(|s| s.projects.iter().flat_map(|p| &p.workspaces).flat_map(|w| &w.tabs).find(|t| t.panes.iter().any(|p| p.id == pane)))
            .map(|t| t.panes.iter().map(|p| p.id).filter(|&id| id != pane).collect())
            .unwrap_or_default();
        if !others.is_empty() {
            let targets = others
                .into_iter()
                .map(|target| {
                    let places = [("Swap with it", "swap"), ("Left of it", "left"), ("Right of it", "right"), ("Above it", "above"), ("Below it", "below")];
                    submenu(
                        label(target),
                        places
                            .into_iter()
                            .map(|(text, place)| {
                                let args = cmd(&["move-pane", "--pane", &p, "--target", &target.to_string(), "--place", place]);
                                item(text, move |this, _, _| this.queue.cmd(args.clone()))
                            })
                            .collect(),
                    )
                })
                .collect();
            entries.push(submenu("Move pane", targets));
        }
        if mouse {
            let (x, y) = cell;
            entries.push(item("Send the right click to the program", move |this, _, _| {
                for kind in ["down", "up"] {
                    this.queue.input(pane, json!({ "mouse": { "kind": kind, "button": "right", "x": x, "y": y } }));
                }
            }));
        }
        let agent = info.as_ref().is_some_and(|i| i.agent.is_some());
        if agent {
            entries.push(Entry::Separator);
            entries.extend(self.agent_menu(pane).into_iter().skip(1));
        }
        entries.push(Entry::Separator);
        let what = info.as_ref().and_then(|i| i.agent.clone().or(i.program.clone())).unwrap_or_else(|| "its shell".into());
        entries.push(danger("Close pane", move |this, window, cx| {
            let args = cmd(&["close", "--pane", &pane.to_string()]);
            if agent {
                dialogs::confirm(
                    "Close this pane?",
                    format!("It runs {what}: closing the pane stops it."),
                    "Close pane",
                    args,
                    false,
                    window,
                    cx,
                );
            } else {
                this.queue.cmd(args);
            }
        }));
        entries
    }

    /// The release of a right press: on another pane it moves the pressed one there, on the
    /// same pane it opens its menu.
    fn pane_release(&mut self, press: PanePress, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let Some((x, y)) = self.cell(position) else { return };
        let target = (x >= 0 && y >= 0).then(|| self.grid.pane_at(x as u16, y as u16).cloned()).flatten();
        match target {
            Some(t) if t.pane != press.pane => {
                let place = pane_place(t.rect, x as u16, y as u16);
                let args = cmd(&["move-pane", "--pane", &press.pane.to_string(), "--target", &t.pane.to_string(), "--place", place]);
                self.queue.cmd(args);
            }
            Some(_) => {
                let entries = self.pane_menu(press.pane, press.cell);
                self.open_menu(position, entries, window, cx);
            }
            None => {}
        }
    }

    pub(crate) fn zoom(&mut self, by: f32, cx: &mut Context<Self>) {
        self.font_size = (self.font_size + by).clamp(8., 32.);
        self.metrics = None;
        cx.notify();
    }

    pub(crate) fn split(&mut self, down: bool) {
        if let Some(pane) = self.active_pane() {
            let mut args = vec!["split".into(), "--pane".into(), pane.to_string(), "--focus".into()];
            if down {
                args.push("--down".into());
            }
            self.act(args);
        }
    }

    pub(crate) fn cycle_tab(&mut self, by: i32) {
        let Some((_, ws)) = self.shown_workspace() else { return };
        let shown = self.session.as_ref().and_then(|s| s.shown().tab);
        let n = ws.tabs.len() as i32;
        if n == 0 {
            return;
        }
        let at = ws.tabs.iter().position(|t| Some(t.id) == shown).unwrap_or(0) as i32;
        let id = ws.tabs[((at + by).rem_euclid(n)) as usize].id;
        self.focus_item("tab", id);
    }

    // -----------------------------------------------------------------------------------------
    // Mouse on the terminal.

    pub(crate) fn cell(&self, position: Point<Pixels>) -> Option<(i32, i32)> {
        Some(self.metrics.as_ref()?.cell_at(self.origin.get()?, position))
    }

    pub(crate) fn on_mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.side.rename.is_some() {
            self.finish_rename(false, window, cx);
        }
        window.focus(&self.focus);
        let Some((x, y)) = self.cell(event.position) else { return };
        if !self.has_frame || x < 0 || y < 0 {
            return;
        }
        let (x, y) = (x as u16, y as u16);
        if let Some(d) = self.grid.dividers.iter().find(|d| d.line.contains(x, y)) {
            // A double click on a divider evens the split out, as on the TUI's borders.
            if event.click_count == 2 {
                let mut args = vec!["ratio".into(), "--tab".into(), self.grid.tab.to_string()];
                for side in &d.path {
                    args.extend(["--path".into(), if *side { "second" } else { "first" }.into()]);
                }
                args.push("0.5".into());
                self.queue.cmd(args);
                return;
            }
            self.drag = Some(Drag::Divider { tab: self.grid.tab, path: d.path.clone(), down: d.dir == "down", area: d.area });
            return;
        }
        let Some(pane) = self.grid.pane_at(x, y).cloned() else { return };
        let px_ = (x - pane.rect.x).min(pane.size.cols.saturating_sub(1));
        let py = (y - pane.rect.y).min(pane.size.rows.saturating_sub(1));
        if event.button == MouseButton::Right {
            // The right button is cornercase's in every pane, even one whose program reads the
            // mouse (almost none uses the right button); its menu can pass the click on.
            self.side.pane_press = Some(PanePress { pane: pane.pane, cell: (px_, py) });
            return;
        }
        if !pane.active {
            self.focus_item("pane", pane.pane);
        }
        if event.modifiers.platform {
            self.open_link(&pane, px_, py, cx);
            return;
        }
        if pane.modes.mouse != MouseMode::None && !event.modifiers.shift {
            let button = match event.button {
                MouseButton::Right => "right",
                MouseButton::Middle => "middle",
                _ => "left",
            };
            self.queue.input(pane.pane, json!({ "mouse": { "kind": "down", "button": button, "x": px_, "y": py } }));
            let motion = matches!(pane.modes.mouse, MouseMode::ButtonMotion | MouseMode::AnyMotion);
            self.drag = Some(Drag::Program { pane: pane.pane, motion });
        } else if event.button == MouseButton::Left {
            if event.click_count >= 2 {
                self.select_by_clicks(&pane, px_, py, event.click_count);
            } else {
                self.drag = Some(Drag::Select { pane: pane.pane, from: (px_, py), to: (px_, py) });
            }
        }
    }

    pub(crate) fn on_mouse_move(&mut self, event: &MouseMoveEvent, _window: &mut Window, _cx: &mut Context<Self>) {
        if event.pressed_button.is_none() {
            return;
        }
        let Some((x, y)) = self.cell(event.position) else { return };
        match &mut self.drag {
            Some(Drag::Program { pane, motion: true }) => {
                let Some(p) = self.grid.pane(*pane) else { return };
                let cx_ = (x - i32::from(p.rect.x)).clamp(0, i32::from(p.size.cols) - 1);
                let cy = (y - i32::from(p.rect.y)).clamp(0, i32::from(p.size.rows) - 1);
                self.queue.input(*pane, json!({ "mouse": { "kind": "drag", "button": "left", "x": cx_, "y": cy } }));
            }
            Some(Drag::Select { pane, from, to }) => {
                let Some(p) = self.grid.pane(*pane) else { return };
                let cx_ = (x - i32::from(p.rect.x)).clamp(0, i32::from(p.size.cols) - 1) as u16;
                let cy = (y - i32::from(p.rect.y)).clamp(0, i32::from(p.size.rows) - 1) as u16;
                if (cx_, cy) != *to {
                    *to = (cx_, cy);
                    let args = vec![
                        "selection".into(),
                        "--pane".into(),
                        pane.to_string(),
                        "--from".into(),
                        format!("{},{}", from.0, from.1),
                        "--to".into(),
                        format!("{cx_},{cy}"),
                    ];
                    self.queue.coalesced("selection", args);
                }
            }
            Some(Drag::Divider { tab, path, down, area }) => {
                let ratio = if *down {
                    (y as f32 - f32::from(area.y)) / f32::from(area.rows.max(1))
                } else {
                    (x as f32 - f32::from(area.x)) / f32::from(area.cols.max(1))
                };
                let mut args = vec!["ratio".into(), "--tab".into(), tab.to_string()];
                for side in path.iter() {
                    args.push("--path".into());
                    args.push(if *side { "second" } else { "first" }.into());
                }
                args.push(format!("{:.3}", ratio.clamp(0.1, 0.9)));
                self.queue.coalesced("ratio", args);
            }
            _ => {}
        }
    }

    pub(crate) fn on_mouse_up(&mut self, event: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.button == MouseButton::Right {
            if let Some(press) = self.side.pane_press.take() {
                self.pane_release(press, event.position, window, cx);
            }
            return;
        }
        match self.drag.take() {
            Some(Drag::Program { pane, .. }) => {
                let Some((x, y)) = self.cell(event.position) else { return };
                let Some(p) = self.grid.pane(pane) else { return };
                let cx_ = (x - i32::from(p.rect.x)).clamp(0, i32::from(p.size.cols) - 1);
                let cy = (y - i32::from(p.rect.y)).clamp(0, i32::from(p.size.rows) - 1);
                self.queue.input(pane, json!({ "mouse": { "kind": "up", "button": "left", "x": cx_, "y": cy } }));
            }
            Some(Drag::Select { pane, from, to }) if from == to => {
                self.queue.cmd(vec!["selection".into(), "--pane".into(), pane.to_string(), "--clear".into()]);
            }
            // A finished selection is copied, as the TUI does.
            Some(Drag::Select { pane, .. }) => self.copy_selection(pane),
            _ => {}
        }
    }

    pub(crate) fn on_scroll(&mut self, event: &ScrollWheelEvent, _window: &mut Window, _cx: &mut Context<Self>) {
        let Some(metrics) = &self.metrics else { return };
        let delta = event.delta.pixel_delta(metrics.line_height);
        self.scroll_rest += f32::from(delta.y) / f32::from(metrics.line_height);
        let lines = self.scroll_rest.trunc() as i32;
        if lines == 0 {
            return;
        }
        self.scroll_rest -= lines as f32;
        let Some((x, y)) = self.cell(event.position) else { return };
        let grid = &self.grid;
        let pane = (x >= 0 && y >= 0)
            .then(|| grid.pane_at(x as u16, y as u16))
            .flatten()
            .or_else(|| grid.active_pane())
            .cloned();
        let Some(pane) = pane else { return };
        let up = lines > 0;
        let n = lines.unsigned_abs().min(25);
        if pane.modes.mouse != MouseMode::None {
            let cx_ = (x - i32::from(pane.rect.x)).clamp(0, i32::from(pane.size.cols) - 1);
            let cy = (y - i32::from(pane.rect.y)).clamp(0, i32::from(pane.size.rows) - 1);
            let kind = if up { "scroll-up" } else { "scroll-down" };
            for _ in 0..n {
                self.queue.input(pane.pane, json!({ "mouse": { "kind": kind, "x": cx_, "y": cy } }));
            }
        } else if pane.modes.alternate_screen {
            if pane.modes.alternate_scroll {
                let code = if up { "up" } else { "down" };
                for _ in 0..n {
                    self.queue.input(pane.pane, json!({ "key": { "code": code } }));
                }
            }
        } else {
            let dir = if up { "--up" } else { "--down" };
            self.queue.cmd(vec!["scroll".into(), "--pane".into(), pane.pane.to_string(), dir.into(), n.to_string()]);
        }
    }

    /// ⌘-click: a hyperlink the program printed, else a path cornercase recognises.
    pub(crate) fn open_link(&mut self, pane: &crate::model::PaneGrid, x: u16, y: u16, cx: &mut Context<Self>) {
        if let Some(link) = pane.cell(x, y).and_then(|(_, run)| run.link.clone()) {
            cx.open_url(&link);
            return;
        }
        let tx = self.tx.clone();
        let args = ["link", "--pane", &pane.pane.to_string(), "--cell", &format!("{x},{y}"), "--json"].map(String::from);
        self.queue.cmd_then(args.to_vec(), move |r| {
            let path = r.ok().and_then(|out| {
                let v: serde_json::Value = serde_json::from_str(&out).ok()?;
                v.get("link")?.get("path")?.as_str().map(String::from)
            });
            match path {
                Some(path) => {
                    let _ = std::process::Command::new("open").arg(&path).spawn();
                }
                None => {
                    let _ = tx.unbounded_send(Msg::Error("No link under the pointer".into()));
                }
            }
        });
    }


    pub(crate) fn render_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        if self.metrics.is_none() {
            self.metrics = Some(Metrics::measure(FONT_FAMILY, self.font_size, window));
        }
        let metrics = self.metrics.clone().expect("measured above");
        let grid = self.grid.clone();
        let has_frame = self.has_frame;
        let origin = self.origin.clone();
        let entity = cx.entity();
        let weak = entity.downgrade();
        let focus = self.focus.clone();
        let focused = focus.is_focused(window) && window.is_window_active();
        let wanted = self.size;

        let surface = canvas(
            move |bounds, _window, cx| {
                let inner = point(bounds.origin.x + px(PADDING), bounds.origin.y + px(PADDING));
                origin.set(Some(inner));
                let w = f32::from(bounds.size.width) - PADDING * 2.;
                let h = f32::from(bounds.size.height) - PADDING * 2.;
                let cols = (w / f32::from(metrics.cell_width)).floor().clamp(1., 1000.) as u16;
                let rows = (h / f32::from(metrics.line_height)).floor().clamp(1., 500.) as u16;
                if wanted != Some((cols, rows)) {
                    let weak = weak.clone();
                    cx.defer(move |cx| {
                        let _ = weak.update(cx, |this, cx| {
                            this.set_size(cols, rows);
                            cx.notify();
                        });
                    });
                }
                (inner, metrics)
            },
            move |bounds, (inner, metrics), window, cx| {
                window.handle_input(&focus, ElementInputHandler::new(bounds, entity), cx);
                if has_frame {
                    render::paint(&grid, inner, &metrics, focused, window, cx);
                }
            },
        )
        .size_full();

        div()
            .id("terminal")
            .flex_1()
            .relative()
            .overflow_hidden()
            .bg(colours::terminal())
            .cursor_text()
            .child(surface)
            .when(!self.has_frame, |d| {
                let text = match &self.conn {
                    Conn::Starting => "Connecting to cornercase…".to_string(),
                    Conn::Connected => "Waiting for the terminal…".to_string(),
                    Conn::Lost(why) => format!("Lost the cornercase server: {why}\nRetrying…"),
                };
                d.child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(colours::muted())
                        .child(text),
                )
            })
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Right, cx.listener(Self::on_mouse_up))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
    }

}

// The platform's input method types text here: composed accents, ⌥ characters, emoji.
impl EntityInputHandler for Root {
    fn text_for_range(
        &mut self,
        _range: Range<usize>,
        _adjusted: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        None
    }

    fn selected_text_range(&mut self, _ignore: bool, _window: &mut Window, _cx: &mut Context<Self>) -> Option<UTF16Selection> {
        let at = self.marked.encode_utf16().count();
        Some(UTF16Selection { range: at..at, reversed: false })
    }

    fn marked_text_range(&self, _window: &mut Window, _cx: &mut Context<Self>) -> Option<Range<usize>> {
        (!self.marked.is_empty()).then(|| 0..self.marked.encode_utf16().count())
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked.clear();
    }

    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        self.marked.clear();
        let text: String = text.chars().filter(|c| !c.is_control()).collect();
        if text.is_empty() {
            return;
        }
        self.input(json!({ "text": text }));
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        _new_selected: Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        self.marked = new_text.to_string();
    }

    fn bounds_for_range(
        &mut self,
        _range: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let metrics = self.metrics.as_ref()?;
        let origin = self.origin.get().unwrap_or(element_bounds.origin);
        let pane = self.grid.active_pane()?;
        let c = pane.cursor?;
        let x = origin.x + metrics.cell_width * f32::from(pane.rect.x + c.x);
        let y = origin.y + metrics.line_height * f32::from(pane.rect.y + c.y);
        Some(Bounds::new(point(x, y), size(metrics.cell_width, metrics.line_height)))
    }

    fn character_index_for_point(
        &mut self,
        _point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pane_dropped_in_the_middle_swaps_else_takes_the_nearest_edge() {
        let rect = Area { x: 10, y: 0, cols: 30, rows: 12 };
        assert_eq!(pane_place(rect, 25, 6), "swap");
        assert_eq!(pane_place(rect, 11, 6), "left");
        assert_eq!(pane_place(rect, 39, 5), "right");
        assert_eq!(pane_place(rect, 24, 0), "above");
        assert_eq!(pane_place(rect, 24, 11), "below");
    }

    #[test]
    fn a_word_is_a_run_of_one_class() {
        let text = "ls ~/src/web-shop && echo";
        let line: Vec<&str> = text.char_indices().map(|(i, c)| &text[i..i + c.len_utf8()]).collect();
        assert_eq!(word_at(&line, 5), (3, 16));
        assert_eq!(word_at(&line, 0), (0, 1));
        assert_eq!(word_at(&line, 18), (18, 19));
        // A wide character's empty second cell is part of it.
        assert_eq!(word_at(&["a", "中", "", "b", " "], 2), (0, 3));
        assert_eq!(word_at(&["a"], 4), (4, 4));
    }
}
