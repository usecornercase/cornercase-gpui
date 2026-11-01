use gpui::{
    Context, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollWheelEvent, Window,
};
use serde_json::{Value, json};

use super::links::{link_args, web_at};
use super::selection::selection_args;
use crate::app::{Drag, Msg, Root};
use crate::model::{Grid, Link, MouseMode, PaneGrid};
use crate::render::layout::{Gap, Hit, Layout};

pub(super) fn button_name(button: MouseButton) -> &'static str {
    match button {
        MouseButton::Right => "right",
        MouseButton::Middle => "middle",
        _ => "left",
    }
}

pub(super) fn mouse(kind: &str, button: &str, (x, y): (u16, u16)) -> Value {
    json!({ "mouse": { "kind": kind, "button": button, "x": x, "y": y } })
}

fn ratio_args(tab: u64, path: &[bool], ratio: String) -> Vec<String> {
    let mut args = vec!["ratio".into(), "--tab".into(), tab.to_string()];
    for side in path {
        args.extend(["--path".into(), if *side { "second" } else { "first" }.into()]);
    }
    args.push(ratio);
    args
}

fn drag_cell(grid: &Grid, layout: &Layout, pane: u64, at: Point<Pixels>) -> Option<(u16, u16)> {
    let (x, y) = layout.cell_in(pane, at)?;
    grid.pane(pane).map(|p| p.clamp_cell(x, y))
}

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "a whole number from 0 to 25")]
pub(super) fn scroll_steps(whole_lines: f32) -> u32 {
    whole_lines.abs().min(25.) as u32
}

pub(super) fn wheel_event(pane: &PaneGrid, (x, y): (i32, i32), up: bool) -> Option<Value> {
    let modes = pane.modes;
    if modes.mouse != MouseMode::None {
        let (x, y) = pane.clamp_cell(x, y);
        let kind = if up { "scroll-up" } else { "scroll-down" };
        return Some(json!({ "mouse": { "kind": kind, "x": x, "y": y } }));
    }
    let code = if up { "up" } else { "down" };
    (modes.alternate_screen && modes.alternate_scroll).then(|| json!({ "key": { "code": code } }))
}

fn unescape(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes.get(i + 1..i + 3).filter(|h| h.iter().all(u8::is_ascii_hexdigit));
        let hex = hex.and_then(|h| std::str::from_utf8(h).ok());
        match (bytes[i], hex.and_then(|h| u8::from_str_radix(h, 16).ok())) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            (byte, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn file_of(uri: &str) -> Option<String> {
    let rest = uri.strip_prefix("file:")?;
    let path = match rest.strip_prefix("//") {
        Some(authority) => &authority[authority.find('/')?..],
        None => rest,
    };
    path.starts_with('/').then(|| unescape(path))
}

impl Root {
    pub(crate) fn on_mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.side.rename.is_some() {
            self.finish_rename(false, window, cx);
        }
        window.focus(&self.focus);
        if !self.frame_shown() {
            return;
        }
        match self.hit(event.position) {
            Some(Hit::Gap(gap)) => {
                self.press_divider(gap, event.click_count);
                cx.notify();
            }
            Some(Hit::Pane(id, (x, y))) => {
                let Some(pane) = self.grid.pane(id).cloned() else { return };
                self.press_pane(&pane, pane.clamp_cell(x, y), event, cx);
            }
            None => {}
        }
    }

    fn press_divider(&mut self, gap: Gap, clicks: usize) {
        let tab = self.grid.tab;
        if clicks == 2 {
            self.queue.cmd(ratio_args(tab, &gap.path, "0.5".into()));
            return;
        }
        self.drag = Some(Drag::Divider { tab, path: gap.path, down: gap.down, along: gap.along });
    }

    fn press_pane(&mut self, pane: &PaneGrid, cell: (u16, u16), event: &MouseDownEvent, cx: &mut Context<Self>) {
        if event.button == MouseButton::Right {
            self.press_right(pane.pane, cell, event.position);
            return;
        }
        if !pane.active {
            self.focus_item("pane", pane.pane);
        }
        if event.modifiers.platform {
            self.open_link(pane, cell.0, cell.1, cx);
            return;
        }
        if self.press_link(pane.pane, cell, event) {
            return;
        }
        if pane.modes.mouse != MouseMode::None && !event.modifiers.shift {
            self.press_program(pane, cell, event.button);
        } else if event.button == MouseButton::Left {
            self.press_select(pane, cell, event.click_count);
        }
    }

    pub(super) fn press_program(&mut self, pane: &PaneGrid, cell: (u16, u16), button: MouseButton) {
        self.queue.input(pane.pane, mouse("down", button_name(button), cell));
        let motion = matches!(pane.modes.mouse, MouseMode::ButtonMotion | MouseMode::AnyMotion);
        self.drag = Some(Drag::Program { pane: pane.pane, motion, cell });
    }

    fn press_select(&mut self, pane: &PaneGrid, cell: (u16, u16), clicks: usize) {
        if clicks >= 2 {
            self.select_by_clicks(pane, cell, clicks);
        } else {
            self.drag = Some(Drag::Select { pane: pane.pane, from: cell, to: cell });
        }
    }

    pub(crate) fn on_mouse_move(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.moves.borrow().pressed().is_some() {
            self.drag_pane(event.position, event.pressed_button, cx);
        } else if event.pressed_button.is_none() {
            self.hover_handle(event.position, cx);
            self.hover_link(event.position, window, cx);
        } else {
            self.drag_to(event.position);
        }
    }

    fn drag_to(&mut self, at: Point<Pixels>) {
        if let Some(Drag::Link { pane, cell }) = self.drag {
            let Some(to) = drag_cell(&self.grid, &self.layout.borrow(), pane, at) else { return };
            self.drag_from_link(pane, cell, to);
        }
        let layout = self.layout.clone();
        let layout = layout.borrow();
        match &mut self.drag {
            Some(Drag::Program { pane, motion: true, cell: last }) => {
                let Some(cell) = drag_cell(&self.grid, &layout, *pane, at) else { return };
                *last = cell;
                self.queue.input(*pane, mouse("drag", "left", cell));
            }
            Some(Drag::Select { pane, from, to }) => {
                let Some(cell) = drag_cell(&self.grid, &layout, *pane, at) else { return };
                if cell != *to {
                    *to = cell;
                    self.queue.coalesced("selection", selection_args(*pane, *from, cell));
                }
            }
            Some(Drag::Divider { tab, path, down, along }) => {
                let ratio = along.ratio(f32::from(if *down { at.y } else { at.x })).clamp(0.1, 0.9);
                self.queue.coalesced("ratio", ratio_args(*tab, path, format!("{ratio:.3}")));
            }
            _ => {}
        }
    }

    pub(crate) fn on_mouse_up(&mut self, event: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.moves.borrow().pressed().is_some() {
            self.release_pane(event, window, cx);
            return;
        }
        if event.button == MouseButton::Right {
            return;
        }
        match self.drag.take() {
            Some(Drag::Program { pane, cell: last, .. }) => {
                let cell = drag_cell(&self.grid, &self.layout.borrow(), pane, event.position).unwrap_or(last);
                self.queue.input(pane, mouse("up", "left", cell));
            }
            Some(Drag::Select { pane, from, to }) if from == to => {
                self.queue.cmd(vec!["selection".into(), "--pane".into(), pane.to_string(), "--clear".into()]);
            }
            Some(Drag::Select { pane, .. }) => self.copy_selection(pane),
            Some(Drag::Divider { .. }) => cx.notify(),
            Some(Drag::Link { pane, cell }) => self.click_link(pane, cell, window, cx),
            None => {}
        }
    }

    pub(crate) fn on_scroll(&mut self, event: &ScrollWheelEvent, _window: &mut Window, cx: &mut Context<Self>) {
        self.drop_link(cx);
        let Some(metrics) = &self.metrics else { return };
        let delta = event.delta.pixel_delta(metrics.line_height);
        self.pointer.scroll_rest += f32::from(delta.y) / f32::from(metrics.line_height);
        let whole_lines = self.pointer.scroll_rest.trunc();
        if whole_lines.abs() < 1. {
            return;
        }
        self.pointer.scroll_rest -= whole_lines;
        let Some((pane, at)) = self.wheel_target(event.position) else { return };
        self.scroll_pane(&pane, at, whole_lines);
    }

    fn wheel_target(&self, position: Point<Pixels>) -> Option<(PaneGrid, (i32, i32))> {
        if let Some(Hit::Pane(id, at)) = self.hit(position) {
            return self.grid.pane(id).map(|pane| (pane.clone(), at));
        }
        let pane = self.grid.active_pane()?.clone();
        let at = self.cell_in(pane.pane, position)?;
        Some((pane, at))
    }

    fn scroll_pane(&self, pane: &PaneGrid, at: (i32, i32), whole_lines: f32) {
        let up = whole_lines > 0.;
        let steps = scroll_steps(whole_lines);
        let rows = if up { i64::from(steps) } else { -i64::from(steps) };
        let Some(event) = wheel_event(pane, at, up) else {
            if !pane.modes.alternate_screen {
                self.queue.scroll(pane.pane, rows);
            }
            return;
        };
        for _ in 0..steps {
            self.queue.input(pane.pane, event.clone());
        }
    }

    fn open_hyperlink(&self, link: &str, cx: &mut Context<Self>) {
        match file_of(link).filter(|_| self.host.is_some()) {
            Some(path) => self.store.update(cx, |s, cx| s.copy_elsewhere(path, cx)),
            None => cx.open_url(link),
        }
    }

    pub(crate) fn open_link(&mut self, pane: &PaneGrid, x: u16, y: u16, cx: &mut Context<Self>) {
        if let Some(link) = pane.cell(x, y).and_then(|(_, run)| run.link.clone()) {
            self.open_hyperlink(&link, cx);
            return;
        }
        if let Some(url) = web_at(pane, (x, y)) {
            cx.open_url(&url);
            return;
        }
        let (tx, remote) = (self.tx.clone(), self.host.is_some());
        self.queue.cmd_then(link_args(pane.pane, (x, y)), move |r| {
            let Some(path) = r.ok().and_then(|out| Link::parse(&out)).map(|link| link.path) else {
                let _ = tx.unbounded_send(Msg::Error("No link under the pointer".into()));
                return;
            };
            if remote {
                let _ = tx.unbounded_send(Msg::Elsewhere(path));
                return;
            }
            let _ = std::process::Command::new("open").arg(&path).spawn();
        });
    }
}

#[cfg(test)]
mod tests;
