use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use gpui::{
    AnyElement, App, Bounds, ClickEvent, Context, CursorStyle, ElementInputHandler, Entity, FocusHandle, Hitbox,
    HitboxBehavior, InteractiveElement, IntoElement, KeyDownEvent, MouseButton, ParentElement, Pixels, Point,
    StatefulInteractiveElement as _, Styled, WeakEntity, Window, canvas, div, prelude::FluentBuilder as _, px,
};

use crate::app::{Conn, Drag, Root};
use crate::dialogs;
use crate::keymap::{self, Command, with_keys};
use crate::keys;
use crate::model::Grid;
use crate::model::link::Hovered;
use crate::panels::{PanelKind, files};
use crate::render::layout::{Hit, Layout};
use crate::render::moving::Moves;
use crate::render::{self, Metrics, Surface};
use crate::store::Store;
use crate::theme::{FONT_FAMILY, colours};
use crate::ui;

mod browser;
mod input;
mod links;
mod mouse;
mod no_tab;
mod pane_drag;
mod pane_menu;
mod pasting;
mod popup;
mod selection;

pub(crate) use links::Pointer;
use popup::Popup;

#[derive(Clone, Copy)]
struct Chord<'a> {
    key: &'a str,
    shift: bool,
    terminal: bool,
    browser: bool,
}

const DEFAULT_FONT_SIZE: f32 = 13.;

fn cycled(ids: &[u64], current: Option<u64>, by: i32) -> Option<u64> {
    let len = i32::try_from(ids.len()).ok().filter(|&len| len > 0)?;
    let at = ids.iter().position(|&id| Some(id) == current).and_then(|i| i32::try_from(i).ok()).unwrap_or(0);
    ids.get(usize::try_from((at + by).rem_euclid(len)).ok()?).copied()
}

fn divider_cursor(down: bool) -> CursorStyle {
    if down { CursorStyle::ResizeRow } else { CursorStyle::ResizeColumn }
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
            let browser = self.browser_has_focus(window, cx);
            let chord = Chord { key: ks.key.as_str(), shift: m.shift, terminal, browser };
            if self.shortcut(chord, window, cx) {
                cx.stop_propagation();
            }
            return;
        }
        if ks.key == "escape" && self.escape(terminal, window, cx) {
            cx.stop_propagation();
            return;
        }
        if !terminal || self.overlay_open(cx) {
            return;
        }
        if let Some(event) = keys::key_event(ks) {
            self.type_in(event, cx);
            cx.stop_propagation();
        }
    }

    fn type_in(&mut self, event: serde_json::Value, cx: &mut Context<Self>) {
        self.dismiss_link(cx);
        let popup = Popup::get(&self.focus, cx);
        if popup.read(cx).is_open() {
            popup.read(cx).send(&event, cx);
        } else {
            self.input(event);
        }
    }

    fn open_popup(&self, cx: &mut App) -> Option<Entity<Popup>> {
        Some(Popup::get(&self.focus, cx)).filter(|popup| popup.read(cx).is_open())
    }

    fn overlay_open(&self, cx: &App) -> bool {
        let s = self.store.read(cx);
        s.dialog.is_some() || s.sheet.is_some()
    }

    fn escape(&mut self, terminal: bool, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.cancel_pane_drag(cx)
            || self.store.update(cx, Store::close_overlay)
            || self.browser_cancel_pick(cx)
            || self.browser_escape(window, cx)
        {
            return true;
        }
        if terminal {
            return false;
        }
        if self.dock_focused(window, cx) {
            self.store.update(cx, Store::close_dock);
        }
        window.focus(&self.focus);
        true
    }

    fn dock_focused(&self, window: &Window, cx: &App) -> bool {
        self.dock_focus.is_focused(window) || self.dock_focus.contains_focused(window, cx)
    }

    fn shortcut(&mut self, chord: Chord<'_>, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(command) = keymap::find(chord.key, chord.shift, chord.browser).map(|s| s.command) else {
            return false;
        };
        let over_dialogs = matches!(
            command,
            Command::Close | Command::Copy | Command::Paste | Command::FindFile | Command::PullRequest
        );
        if self.store.read(cx).dialog.is_some() && !over_dialogs {
            return true;
        }
        if chord.browser && self.browser_command(command, window, cx) {
            return true;
        }
        match command {
            Command::Panel(kind) => self.store.update(cx, |s, cx| s.toggle(kind, cx)),
            Command::FindFile => self.find_file(window, cx),
            Command::Copy | Command::Paste => self.clipboard_shortcut(command, chord.terminal, cx),
            Command::Close => self.close_shortcut(window, cx),
            Command::CloseTab => self.close_shown_tab(window, cx),
            Command::Shortcuts => dialogs::shortcuts(window, cx),
            Command::OpenProject => self.open_project(window, cx),
            Command::Connect => dialogs::connect(window, cx),
            Command::NewWorkspace => dialogs::new_workspace(None, window, cx),
            Command::StartAgent => dialogs::start_agent(window, cx),
            Command::PullRequest => dialogs::shown_pull_request(window, cx),
            _ => self.view_shortcut(command, chord.key, cx),
        }
        true
    }

    fn find_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.store.read(cx).dialog.clone() {
            Some(dialog) if files::is_finder(&dialog) => {
                self.store.update(cx, Store::close_dialog);
            }
            Some(_) => {}
            None => {
                let panel = self.panel(PanelKind::Files, window, cx);
                files::find_file(panel, window, cx);
            }
        }
    }

    fn clipboard_shortcut(&mut self, command: Command, terminal: bool, cx: &mut Context<Self>) {
        if let Some(popup) = self.open_popup(cx) {
            if terminal && command == Command::Paste {
                popup.read(cx).paste(cx);
            }
            return;
        }
        let Some(pane) = self.active_pane().filter(|_| terminal) else { return };
        if command == Command::Copy {
            self.copy_selection(pane);
        } else {
            self.paste(pane, cx);
        }
    }

    fn view_shortcut(&mut self, command: Command, key: &str, cx: &mut Context<Self>) {
        match command {
            Command::NewTab => self.act(vec!["new-tab".into(), "--focus".into()]),
            Command::SplitRight => self.split(false),
            Command::SplitDown => self.split(true),
            Command::PreviousTab => self.cycle_tab(-1),
            Command::NextTab => self.cycle_tab(1),
            Command::Tab => self.focus_tab_number(key),
            Command::PreviousPane => self.cycle_pane(-1),
            Command::NextPane => self.cycle_pane(1),
            Command::ZoomIn => self.zoom(1., cx),
            Command::ZoomOut => self.zoom(-1., cx),
            Command::ZoomReset => self.set_font_size(DEFAULT_FONT_SIZE, cx),
            _ => {}
        }
    }

    fn close_shortcut(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.store.update(cx, Store::close_overlay) {
            return;
        }
        if let Some(popup) = self.open_popup(cx) {
            popup.read(cx).close(cx);
            return;
        }
        if self.dock_focused(window, cx) && self.store.update(cx, Store::close_dock) {
            window.focus(&self.focus);
            return;
        }
        if let Some(pane) = self.active_pane() {
            self.close_pane(pane, window, cx);
        }
    }

    fn close_shown_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let session = self.session.as_ref();
        let tab = session.and_then(|s| s.tab(s.shown().tab?)).cloned();
        if let Some(tab) = tab {
            self.close_tab(&tab, window, cx);
        }
    }

    fn focus_tab_number(&mut self, key: &str) {
        let Some((_, ws)) = self.shown_workspace() else { return };
        if let Some(tab) = keymap::tab_index(key, ws.tabs.len()).and_then(|i| ws.tabs.get(i)).map(|t| t.id) {
            self.focus_item("tab", tab);
        }
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
        let ids: Vec<u64> = ws.tabs.iter().map(|t| t.id).collect();
        let shown = self.session.as_ref().and_then(|s| s.shown().tab);
        if let Some(tab) = cycled(&ids, shown, by) {
            self.focus_item("tab", tab);
        }
    }

    fn cycle_pane(&mut self, by: i32) {
        let Some(pane) = self.active_pane() else { return };
        let tab = self.session.as_ref().and_then(|s| s.tab_of_pane(pane));
        let ids: Vec<u64> = tab.map(|t| t.panes.iter().map(|p| p.id).collect()).unwrap_or_default();
        if let Some(next) = cycled(&ids, Some(pane), by).filter(|&next| next != pane) {
            self.focus_item("pane", next);
        }
    }

    pub(crate) fn hit(&self, position: Point<Pixels>) -> Option<Hit> {
        self.layout.borrow().hit(position)
    }

    pub(crate) fn cell_in(&self, pane: u64, position: Point<Pixels>) -> Option<(i32, i32)> {
        self.layout.borrow().cell_in(pane, position)
    }

    pub(crate) fn render_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        if self.metrics.is_none() {
            self.metrics = Some(Metrics::measure(FONT_FAMILY, self.font_size, window));
        }
        let metrics = self.metrics.clone().expect("measured above");
        let popup = Popup::get(&self.focus, cx);
        popup.update(cx, |popup, cx| popup.set_metrics(&metrics, cx));
        let handle = self.render_handle(cx).filter(|_| !popup.read(cx).is_open());
        div()
            .id("terminal")
            .flex_1()
            .relative()
            .overflow_hidden()
            .cursor(self.pointer())
            .child(self.surface(metrics.clone(), window, cx))
            .children(handle)
            .children(self.cover(cx))
            .child(popup)
            .child(files::Peek::get(cx))
            .children(self.render_ghost(&metrics))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| this.left_terminal(*hovered, cx)))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_mouse_down))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Right, cx.listener(Self::on_mouse_up))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .on_drop(cx.listener(Self::drop_paths))
    }

    fn cover(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.conn == Conn::Connected && self.shows_no_tab() {
            return self.empty_page(cx).map(IntoElement::into_any_element);
        }
        (!self.has_frame).then(|| self.connection_note().into_any_element())
    }

    fn connection_note(&self) -> impl IntoElement + use<> {
        let (title, detail) = note_text(&self.conn, self.surface_error.as_deref(), self.host.as_deref());
        let lost_remote = self.host.is_some() && matches!(self.conn, Conn::Lost(_));
        div()
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(6.))
            .p(px(24.))
            .child(ui::brand::mark(5., matches!(self.conn, Conn::Starting)).mb(px(22.)))
            .child(div().text_color(colours::soft()).child(title))
            .when_some(detail, |d, why| {
                d.child(div().max_w(px(460.)).text_center().text_size(px(12.)).text_color(colours::faint()).child(why))
                    .child(div().text_size(px(11.)).text_color(colours::faint()).child("Retrying…"))
            })
            .when(lost_remote, |d| d.child(elsewhere_button()))
    }

    fn pointer(&self) -> CursorStyle {
        if self.moves.borrow().lifted() {
            return CursorStyle::ClosedHand;
        }
        match &self.drag {
            Some(Drag::Divider { down, .. }) => divider_cursor(*down),
            Some(Drag::Link { .. }) => CursorStyle::PointingHand,
            None if self.shown_link().is_some() => CursorStyle::PointingHand,
            _ => CursorStyle::IBeam,
        }
    }

    fn shown_link(&self) -> Option<Hovered> {
        self.pointer.links.hovered().filter(|h| h.shown_in(&self.grid)).cloned()
    }

    fn surface(&self, metrics: Metrics, window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let entity = cx.entity();
        let shown = self.frame_shown();
        let grid = shown.then(|| self.grid.clone());
        let (layout, moves) = (self.layout.clone(), self.moves.clone());
        let fit = Fit { layout, moves: moves.clone(), wanted: self.size, root: entity.downgrade(), metrics, grid };
        let focused = self.focus.is_focused(window) && window.is_window_active();
        let (grid, has_frame, focus) = (self.grid.clone(), shown, self.focus.clone());
        let link = self.shown_link().filter(|_| self.drag.is_none() || matches!(self.drag, Some(Drag::Link { .. })));
        let painting = Painting { grid, has_frame, focus, focused, entity, moves, link };
        canvas(
            move |bounds, window, cx| fit.prepaint(bounds, window, cx),
            move |bounds, placed, window, cx| {
                painting.paint(bounds, &placed, window, cx);
            },
        )
        .size_full()
    }
}

struct Fit {
    layout: Rc<RefCell<Layout>>,
    moves: Rc<RefCell<Moves>>,
    wanted: Option<(u16, u16)>,
    root: WeakEntity<Root>,
    metrics: Metrics,
    grid: Option<Rc<Grid>>,
}

struct Placed {
    layout: Layout,
    metrics: Metrics,
    grips: Vec<(Hitbox, CursorStyle)>,
}

impl Fit {
    fn prepaint(self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) -> Placed {
        let cell = self.metrics.cell();
        let layout = self.grid.as_deref().map(|grid| Layout::new(grid, cell, bounds)).unwrap_or_default();
        let extra = Layout::extra(self.grid.as_deref().unwrap_or(&Grid::default()), cell);
        let before = self.layout.replace(layout.clone());
        self.moves.borrow_mut().laid_out(&before, &layout, Instant::now());
        let grips = layout
            .gaps
            .iter()
            .map(|gap| (window.insert_hitbox(gap.bounds, HitboxBehavior::Normal), divider_cursor(gap.down)))
            .collect();
        let cols = cells_across(f32::from(bounds.size.width) - extra.0, self.metrics.cell_width, 1000.);
        let rows = cells_across(f32::from(bounds.size.height) - extra.1, self.metrics.line_height, 500.);
        if self.wanted != Some((cols, rows)) {
            request_size(self.root, cols, rows, cx);
        }
        Placed { layout, metrics: self.metrics, grips }
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
    moves: Rc<RefCell<Moves>>,
    link: Option<Hovered>,
}

impl Painting {
    fn paint(self, bounds: Bounds<Pixels>, placed: &Placed, window: &mut Window, cx: &mut App) {
        window.handle_input(&self.focus, ElementInputHandler::new(bounds, self.entity.clone()), cx);
        for (hitbox, cursor) in &placed.grips {
            window.set_cursor_style(*cursor, hitbox);
        }
        if !self.has_frame {
            render::card(bounds, colours::pane(), colours::border(), window);
            return;
        }
        let now = Instant::now();
        let (stage, moving) = {
            let mut moves = self.moves.borrow_mut();
            let was = moves.busy(now);
            moves.tick(now);
            (moves.stage(&placed.layout, now), was || moves.busy(now))
        };
        let surface =
            Surface { layout: &placed.layout, metrics: &placed.metrics, focused: self.focused, stage: &stage };
        render::paint(&self.grid, &surface, window, cx);
        if let Some(link) = self.link.as_ref().filter(|_| !moving) {
            render::paint_link(&placed.layout, link, window);
        }
        pane_drag::follow(&self.moves, &self.entity, moving, window);
    }
}

fn note_text(conn: &Conn, surface_error: Option<&str>, host: Option<&str>) -> (String, Option<String>) {
    let server = host.map_or_else(|| "cornercase".to_string(), |host| format!("cornercase on {host}"));
    match (conn, surface_error) {
        (Conn::Starting, _) => (format!("Connecting to {server}…"), None),
        (Conn::Connected, None) => ("Waiting for the terminal…".to_string(), None),
        (Conn::Connected, Some(why)) => ("The terminal cannot be shown".to_string(), Some(why.to_string())),
        (Conn::Lost(why), _) => (format!("Lost {server}"), Some(why.clone())),
    }
}

fn elsewhere_button() -> impl IntoElement {
    ui::button("note-connect", with_keys("Connect to another server", Command::Connect))
        .mt(px(10.))
        .on_click(|_: &ClickEvent, window, cx| dialogs::connect(window, cx))
}

#[cfg(test)]
mod tests;
