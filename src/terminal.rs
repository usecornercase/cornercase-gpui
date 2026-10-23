use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    App, Bounds, Context, ElementInputHandler, Entity, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    MouseButton, ParentElement, Pixels, Point, Styled, WeakEntity, Window, canvas, div, point,
    prelude::FluentBuilder as _, px,
};

use crate::app::{Conn, PADDING, Root};
use crate::dialogs;
use crate::keys;
use crate::model::Grid;
use crate::panels::PanelKind;
use crate::render::{self, Metrics, Surface};
use crate::theme::{FONT_FAMILY, colours};

mod input;
mod mouse;
mod pane_menu;
mod selection;

pub(crate) struct PanePress {
    pub pane: u64,
    pub cell: (u16, u16),
}

#[derive(Clone, Copy)]
struct Chord<'a> {
    key: &'a str,
    shift: bool,
    terminal: bool,
}

#[derive(Clone, Copy)]
enum Opening {
    Docked,
    Sheet,
}

const TAB_KEYS: [&str; 9] = ["1", "2", "3", "4", "5", "6", "7", "8", "9"];
const DEFAULT_FONT_SIZE: f32 = 13.;

fn panel_shortcut(chord: Chord<'_>) -> Option<(PanelKind, Opening)> {
    let shift = chord.shift;
    Some(match chord.key {
        "c" if shift => (PanelKind::Changes, Opening::Docked),
        "e" if shift => (PanelKind::Files, Opening::Docked),
        "l" if shift => (PanelKind::Todo, Opening::Docked),
        "," => (PanelKind::Settings, Opening::Sheet),
        "i" => (PanelKind::Issues, Opening::Sheet),
        "u" => (PanelKind::Usage, Opening::Sheet),
        "f" if shift => (PanelKind::Search, Opening::Sheet),
        _ => return None,
    })
}

fn surface_cell((x, y): (i32, i32)) -> Option<(u16, u16)> {
    Some((u16::try_from(x).ok()?, u16::try_from(y).ok()?))
}

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "clamped to 1..=max before the cast")]
fn cells_across(space: f32, cell: Pixels, max: f32) -> u16 {
    (space / f32::from(cell)).floor().clamp(1., max) as u16
}

impl Root {
    pub(crate) fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let ks = &event.keystroke;
        let m = ks.modifiers;
        let terminal = self.focus.is_focused(window);
        if m.platform {
            let chord = Chord { key: ks.key.as_str(), shift: m.shift, terminal };
            if self.shortcut(chord, window, cx) {
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

    fn shortcut(&mut self, chord: Chord<'_>, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if let Some((kind, opening)) = panel_shortcut(chord) {
            self.store.update(cx, |s, cx| match opening {
                Opening::Docked => s.toggle_dock(kind, cx),
                Opening::Sheet => s.open(kind, cx),
            });
            return true;
        }
        self.clipboard_shortcut(chord, cx) || self.window_shortcut(chord, window, cx) || self.view_shortcut(chord, cx)
    }

    fn clipboard_shortcut(&mut self, chord: Chord<'_>, cx: &mut Context<Self>) -> bool {
        if !chord.terminal || !matches!(chord.key, "c" | "v") {
            return false;
        }
        let Some(pane) = self.active_pane() else { return true };
        if chord.key == "c" {
            self.copy_selection(pane);
        } else {
            self.paste(pane, cx);
        }
        true
    }

    fn window_shortcut(&mut self, chord: Chord<'_>, window: &mut Window, cx: &mut Context<Self>) -> bool {
        match chord.key {
            "p" if !chord.shift => self.focus_filter(window, cx),
            "t" => self.act(vec!["new-tab".into(), "--focus".into()]),
            "d" => self.split(chord.shift),
            "o" => self.open_project(window, cx),
            "n" => dialogs::new_workspace(None, window, cx),
            "g" => dialogs::start_agent(window, cx),
            _ => return false,
        }
        true
    }

    fn view_shortcut(&mut self, chord: Chord<'_>, cx: &mut Context<Self>) -> bool {
        match chord.key {
            "=" | "+" => self.zoom(1., cx),
            "-" => self.zoom(-1., cx),
            "0" => self.set_font_size(DEFAULT_FONT_SIZE, cx),
            "[" if chord.shift => self.cycle_tab(-1),
            "]" if chord.shift => self.cycle_tab(1),
            key => return self.focus_tab_number(key),
        }
        true
    }

    fn focus_tab_number(&mut self, key: &str) -> bool {
        let Some(index) = TAB_KEYS.iter().position(|k| *k == key) else { return false };
        if let Some(tab) = self.shown_workspace().and_then(|(_, w)| w.tabs.get(index)).map(|t| t.id) {
            self.focus_item("tab", tab);
        }
        true
    }

    pub(crate) fn zoom(&mut self, by: f32, cx: &mut Context<Self>) {
        self.set_font_size((self.font_size + by).clamp(8., 32.), cx);
    }

    fn set_font_size(&mut self, size: f32, cx: &mut Context<Self>) {
        self.font_size = size;
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
        let Ok(len) = i32::try_from(ws.tabs.len()) else { return };
        if len == 0 {
            return;
        }
        let at = ws.tabs.iter().position(|t| Some(t.id) == shown).and_then(|i| i32::try_from(i).ok()).unwrap_or(0);
        let Some(tab) = usize::try_from((at + by).rem_euclid(len)).ok().and_then(|i| ws.tabs.get(i)) else { return };
        let id = tab.id;
        self.focus_item("tab", id);
    }

    pub(crate) fn cell(&self, position: Point<Pixels>) -> Option<(i32, i32)> {
        Some(self.metrics.as_ref()?.cell_at(self.origin.get()?, position))
    }

    pub(crate) fn render_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        if self.metrics.is_none() {
            self.metrics = Some(Metrics::measure(FONT_FAMILY, self.font_size, window));
        }
        let metrics = self.metrics.clone().expect("measured above");
        div()
            .id("terminal")
            .flex_1()
            .relative()
            .overflow_hidden()
            .bg(colours::terminal())
            .cursor_text()
            .child(self.surface(metrics, window, cx))
            .when(!self.has_frame, |d| d.child(connection_note(&self.conn)))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Right, cx.listener(Self::on_mouse_up))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
    }

    fn surface(&self, metrics: Metrics, window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let entity = cx.entity();
        let fit = Fit { origin: self.origin.clone(), wanted: self.size, root: entity.downgrade(), metrics };
        let focused = self.focus.is_focused(window) && window.is_window_active();
        let painting =
            Painting { grid: self.grid.clone(), has_frame: self.has_frame, focus: self.focus.clone(), focused, entity };
        canvas(
            move |bounds, _, cx| fit.prepaint(bounds, cx),
            move |bounds, placed, window, cx| {
                painting.paint(bounds, &placed, window, cx);
            },
        )
        .size_full()
    }
}

struct Fit {
    origin: Rc<Cell<Option<Point<Pixels>>>>,
    wanted: Option<(u16, u16)>,
    root: WeakEntity<Root>,
    metrics: Metrics,
}

struct Placed {
    inner: Point<Pixels>,
    metrics: Metrics,
}

impl Fit {
    fn prepaint(self, bounds: Bounds<Pixels>, cx: &mut App) -> Placed {
        let inner = point(bounds.origin.x + px(PADDING), bounds.origin.y + px(PADDING));
        self.origin.set(Some(inner));
        let cols = cells_across(f32::from(bounds.size.width) - PADDING * 2., self.metrics.cell_width, 1000.);
        let rows = cells_across(f32::from(bounds.size.height) - PADDING * 2., self.metrics.line_height, 500.);
        if self.wanted != Some((cols, rows)) {
            request_size(self.root, cols, rows, cx);
        }
        Placed { inner, metrics: self.metrics }
    }
}

fn request_size(root: WeakEntity<Root>, cols: u16, rows: u16, cx: &mut App) {
    cx.defer(move |cx| {
        let _ = root.update(cx, |this, cx| {
            this.set_size(cols, rows);
            cx.notify();
        });
    });
}

struct Painting {
    grid: Rc<Grid>,
    has_frame: bool,
    focus: FocusHandle,
    focused: bool,
    entity: Entity<Root>,
}

impl Painting {
    fn paint(self, bounds: Bounds<Pixels>, placed: &Placed, window: &mut Window, cx: &mut App) {
        window.handle_input(&self.focus, ElementInputHandler::new(bounds, self.entity), cx);
        if self.has_frame {
            let surface = Surface { origin: placed.inner, metrics: &placed.metrics, focused: self.focused };
            render::paint(&self.grid, &surface, window, cx);
        }
    }
}

fn connection_note(conn: &Conn) -> impl IntoElement + use<> {
    let text = match conn {
        Conn::Starting => "Connecting to cornercase…".to_string(),
        Conn::Connected => "Waiting for the terminal…".to_string(),
        Conn::Lost(why) => format!("Lost the cornercase server: {why}\nRetrying…"),
    };
    div().absolute().inset_0().flex().items_center().justify_center().text_color(colours::muted()).child(text)
}

#[cfg(test)]
mod tests;
