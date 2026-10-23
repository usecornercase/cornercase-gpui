use gpui::{Context, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ScrollWheelEvent, Window};
use serde_json::{Value, json};

use super::selection::selection_args;
use super::{PanePress, surface_cell};
use crate::app::{Drag, Msg, Root};
use crate::model::{Area, Divider, MouseMode, PaneGrid};

fn button_name(button: MouseButton) -> &'static str {
    match button {
        MouseButton::Right => "right",
        MouseButton::Middle => "middle",
        _ => "left",
    }
}

fn mouse(kind: &str, button: &str, (x, y): (u16, u16)) -> Value {
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

fn divider_ratio(down: bool, area: Area, (x, y): (i32, i32)) -> f64 {
    if down {
        (f64::from(y) - f64::from(area.y)) / f64::from(area.rows.max(1))
    } else {
        (f64::from(x) - f64::from(area.x)) / f64::from(area.cols.max(1))
    }
}

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "a whole number from 0 to 25")]
fn scroll_steps(whole_lines: f32) -> u32 {
    whole_lines.abs().min(25.) as u32
}

fn wheel_event(pane: &PaneGrid, (x, y): (i32, i32), up: bool) -> Option<Value> {
    let modes = pane.modes;
    if modes.mouse != MouseMode::None {
        let (x, y) = pane.clamp_cell(x, y);
        let kind = if up { "scroll-up" } else { "scroll-down" };
        return Some(json!({ "mouse": { "kind": kind, "x": x, "y": y } }));
    }
    let code = if up { "up" } else { "down" };
    (modes.alternate_screen && modes.alternate_scroll).then(|| json!({ "key": { "code": code } }))
}

fn scroll_args(pane: u64, up: bool, steps: u32) -> Vec<String> {
    let dir = if up { "--up" } else { "--down" };
    vec!["scroll".into(), "--pane".into(), pane.to_string(), dir.into(), steps.to_string()]
}

fn link_path(out: &str) -> Option<String> {
    let value: Value = serde_json::from_str(out).ok()?;
    value.get("link")?.get("path")?.as_str().map(String::from)
}

impl Root {
    pub(crate) fn on_mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.side.rename.is_some() {
            self.finish_rename(false, window, cx);
        }
        window.focus(&self.focus);
        let Some((x, y)) = self.cell(event.position).filter(|_| self.has_frame).and_then(surface_cell) else {
            return;
        };
        if let Some(divider) = self.grid.dividers.iter().find(|d| d.line.contains(x, y)).cloned() {
            self.press_divider(divider, event.click_count);
            return;
        }
        let Some(pane) = self.grid.pane_at(x, y).cloned() else { return };
        self.press_pane(&pane, (x, y), event, cx);
    }

    fn press_divider(&mut self, divider: Divider, clicks: usize) {
        let tab = self.grid.tab;
        if clicks == 2 {
            self.queue.cmd(ratio_args(tab, &divider.path, "0.5".into()));
            return;
        }
        self.drag = Some(Drag::Divider { tab, path: divider.path, down: divider.dir == "down", area: divider.area });
    }

    fn press_pane(&mut self, pane: &PaneGrid, (x, y): (u16, u16), event: &MouseDownEvent, cx: &mut Context<Self>) {
        let cell = (
            (x - pane.rect.x).min(pane.size.cols.saturating_sub(1)),
            (y - pane.rect.y).min(pane.size.rows.saturating_sub(1)),
        );
        if event.button == MouseButton::Right {
            self.side.pane_press = Some(PanePress { pane: pane.pane, cell });
            return;
        }
        if !pane.active {
            self.focus_item("pane", pane.pane);
        }
        if event.modifiers.platform {
            self.open_link(pane, cell.0, cell.1, cx);
            return;
        }
        if pane.modes.mouse != MouseMode::None && !event.modifiers.shift {
            self.queue.input(pane.pane, mouse("down", button_name(event.button), cell));
            let motion = matches!(pane.modes.mouse, MouseMode::ButtonMotion | MouseMode::AnyMotion);
            self.drag = Some(Drag::Program { pane: pane.pane, motion });
        } else if event.button == MouseButton::Left {
            self.press_select(pane, cell, event.click_count);
        }
    }

    fn press_select(&mut self, pane: &PaneGrid, cell: (u16, u16), clicks: usize) {
        if clicks >= 2 {
            self.select_by_clicks(pane, cell, clicks);
        } else {
            self.drag = Some(Drag::Select { pane: pane.pane, from: cell, to: cell });
        }
    }

    pub(crate) fn on_mouse_move(&mut self, event: &MouseMoveEvent, _window: &mut Window, _cx: &mut Context<Self>) {
        if event.pressed_button.is_none() {
            return;
        }
        let Some((x, y)) = self.cell(event.position) else { return };
        match &mut self.drag {
            Some(Drag::Program { pane, motion: true }) => {
                let Some(cell) = self.grid.pane(*pane).map(|p| p.clamp_cell(x, y)) else { return };
                self.queue.input(*pane, mouse("drag", "left", cell));
            }
            Some(Drag::Select { pane, from, to }) => {
                let Some(cell) = self.grid.pane(*pane).map(|p| p.clamp_cell(x, y)) else { return };
                if cell != *to {
                    *to = cell;
                    self.queue.coalesced("selection", selection_args(*pane, *from, cell));
                }
            }
            Some(Drag::Divider { tab, path, down, area }) => {
                let ratio = divider_ratio(*down, *area, (x, y)).clamp(0.1, 0.9);
                self.queue.coalesced("ratio", ratio_args(*tab, path, format!("{ratio:.3}")));
            }
            _ => {}
        }
    }

    pub(crate) fn on_mouse_up(&mut self, event: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.button == MouseButton::Right {
            if let Some(press) = self.side.pane_press.take() {
                self.pane_release(&press, event.position, window, cx);
            }
            return;
        }
        match self.drag.take() {
            Some(Drag::Program { pane, .. }) => {
                let Some((x, y)) = self.cell(event.position) else { return };
                let Some(cell) = self.grid.pane(pane).map(|p| p.clamp_cell(x, y)) else { return };
                self.queue.input(pane, mouse("up", "left", cell));
            }
            Some(Drag::Select { pane, from, to }) if from == to => {
                self.queue.cmd(vec!["selection".into(), "--pane".into(), pane.to_string(), "--clear".into()]);
            }
            Some(Drag::Select { pane, .. }) => self.copy_selection(pane),
            _ => {}
        }
    }

    pub(crate) fn on_scroll(&mut self, event: &ScrollWheelEvent, _window: &mut Window, _cx: &mut Context<Self>) {
        let Some(metrics) = &self.metrics else { return };
        let delta = event.delta.pixel_delta(metrics.line_height);
        self.scroll_rest += f32::from(delta.y) / f32::from(metrics.line_height);
        let whole_lines = self.scroll_rest.trunc();
        if whole_lines.abs() < 1. {
            return;
        }
        self.scroll_rest -= whole_lines;
        let Some(at) = self.cell(event.position) else { return };
        let grid = &self.grid;
        let pane = surface_cell(at).and_then(|(x, y)| grid.pane_at(x, y)).or_else(|| grid.active_pane()).cloned();
        let Some(pane) = pane else { return };
        self.scroll_pane(&pane, at, whole_lines);
    }

    fn scroll_pane(&self, pane: &PaneGrid, at: (i32, i32), whole_lines: f32) {
        let up = whole_lines > 0.;
        let steps = scroll_steps(whole_lines);
        let Some(event) = wheel_event(pane, at, up) else {
            if !pane.modes.alternate_screen {
                self.queue.cmd(scroll_args(pane.pane, up, steps));
            }
            return;
        };
        for _ in 0..steps {
            self.queue.input(pane.pane, event.clone());
        }
    }

    pub(crate) fn open_link(&mut self, pane: &PaneGrid, x: u16, y: u16, cx: &mut Context<Self>) {
        if let Some(link) = pane.cell(x, y).and_then(|(_, run)| run.link.clone()) {
            cx.open_url(&link);
            return;
        }
        let tx = self.tx.clone();
        let args =
            ["link", "--pane", &pane.pane.to_string(), "--cell", &format!("{x},{y}"), "--json"].map(String::from);
        self.queue.cmd_then(args.to_vec(), move |r| {
            let Some(path) = r.ok().and_then(|out| link_path(&out)) else {
                let _ = tx.unbounded_send(Msg::Error("No link under the pointer".into()));
                return;
            };
            let _ = std::process::Command::new("open").arg(&path).spawn();
        });
    }
}
