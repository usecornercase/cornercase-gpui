//! The terminal: drawing the surface, and the keys, text, mouse and wheel that go to its panes.

use std::ops::Range;

use gpui::{
    Bounds, Context, ElementInputHandler, EntityInputHandler, InteractiveElement, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, ScrollWheelEvent,
    Styled, UTF16Selection, Window, canvas, div, point, prelude::FluentBuilder as _,
    px, size,
};
use serde_json::json;

use crate::app::{Conn, Drag, FONT_FAMILY, Msg, PADDING, Root};
use crate::dialogs;
use crate::keys;
use crate::model::MouseMode;
use crate::panels::PanelKind;
use crate::render::{self, Metrics};
use crate::theme::colours;

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
            }
            "v" if terminal => {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    self.input(json!({ "paste": text }));
                }
            }
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
        window.focus(&self.focus);
        self.confirm_close = None;
        let Some((x, y)) = self.cell(event.position) else { return };
        if !self.has_frame || x < 0 || y < 0 {
            return;
        }
        let (x, y) = (x as u16, y as u16);
        if let Some(d) = self.grid.dividers.iter().find(|d| d.line.contains(x, y)) {
            self.drag = Some(Drag::Divider { tab: self.grid.tab, path: d.path.clone(), down: d.dir == "down", area: d.area });
            return;
        }
        let Some(pane) = self.grid.pane_at(x, y).cloned() else { return };
        if !pane.active {
            self.focus_item("pane", pane.pane);
        }
        let px_ = (x - pane.rect.x).min(pane.size.cols.saturating_sub(1));
        let py = (y - pane.rect.y).min(pane.size.rows.saturating_sub(1));
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
            self.drag = Some(Drag::Select { pane: pane.pane, from: (px_, py), to: (px_, py) });
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

    pub(crate) fn on_mouse_up(&mut self, event: &MouseUpEvent, _window: &mut Window, _cx: &mut Context<Self>) {
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
        cx: &mut Context<Self>,
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
