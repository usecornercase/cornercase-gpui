use std::cell::RefCell;
use std::time::Instant;

use gpui::{
    AnyElement, Context, CursorStyle, DispatchPhase, Entity, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, SharedString, Size,
    StatefulInteractiveElement as _, Styled, Window, canvas, deferred, div, px,
};

use super::pane_menu::move_pane_args;
use crate::app::Root;
use crate::render::layout::Hit;
use crate::render::moving::{self, Aim, Answer, Grip, Moves, Press, Release};
use crate::render::{self, Metrics};
use crate::sidebar::tip;
use crate::store;
use crate::theme::colours;
use crate::ui::glass::tint;
use crate::ui::motion::{self, Way};

const PILL: (f32, f32) = (30., 4.);
const PILL_HOVER: f32 = 44.;

fn handle(pane: u64, (at, area): (Point<Pixels>, Size<Pixels>), cx: &mut Context<Root>) -> impl IntoElement + use<> {
    let group = SharedString::from(format!("pane-handle-{pane}"));
    let pill = div()
        .w(px(PILL.0))
        .h(px(PILL.1))
        .rounded_full()
        .bg(tint(colours::text(), 0.34))
        .group_hover(group.clone(), |s| s.w(px(PILL_HOVER)).bg(tint(colours::text(), 0.8)));
    let grip = div()
        .id(group.clone())
        .group(group)
        .absolute()
        .left(at.x)
        .top(at.y)
        .w(area.width)
        .h(area.height)
        .flex()
        .items_center()
        .justify_center()
        .cursor(CursorStyle::OpenHand)
        .tooltip(tip("Drag to move this pane"))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, e: &MouseDownEvent, window, cx| this.press_handle(pane, e, window, cx)),
        )
        .child(pill);
    motion::fade(grip, SharedString::from(format!("pane-handle-in-{pane}")), Way::In)
}

pub(super) fn follow(moves: &RefCell<Moves>, root: &Entity<Root>, moving: bool, window: &mut Window) {
    let moves = moves.borrow();
    if moving {
        window.request_animation_frame();
    }
    if moves.pressed().is_none() {
        return;
    }
    if moves.lifted() {
        window.set_window_cursor_style(CursorStyle::ClosedHand);
    }
    listen(root, window);
}

fn send_move(pane: u64, aim: Aim, cx: &mut Context<Root>) {
    let root = cx.entity().downgrade();
    store::queued(cx, move_pane_args(pane, aim), move |result, cx| {
        let _ = root.update(cx, |this, cx| this.move_answered(&result, cx));
    });
}

fn listen(root: &Entity<Root>, window: &mut Window) {
    let (moved, released) = (root.downgrade(), root.downgrade());
    window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
        if phase == DispatchPhase::Capture {
            cx.stop_propagation();
            let _ = moved.update(cx, |this, cx| this.drag_pane(event.position, event.pressed_button, cx));
        }
    });
    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
        if phase == DispatchPhase::Capture {
            cx.stop_propagation();
            let _ = released.update(cx, |this, cx| this.release_pane(event, window, cx));
        }
    });
}

impl Root {
    pub(super) fn hover_handle(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        let near = if self.frame_shown() { moving::near_top(&self.layout.borrow(), at) } else { None };
        if self.moves.borrow_mut().hover(near) {
            cx.notify();
        }
    }

    pub(super) fn left_terminal(&mut self, hovered: bool, cx: &mut Context<Self>) {
        if hovered {
            return;
        }
        if self.moves.borrow_mut().hover(None) {
            cx.notify();
        }
        self.forget_link(cx);
    }

    fn press_handle(&mut self, pane: u64, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        if self.side.rename.is_some() {
            self.finish_rename(false, window, cx);
        }
        window.focus(&self.focus);
        self.moves.borrow_mut().press(Press { pane, grip: Grip::Handle, cell: (0, 0), at: event.position });
        cx.notify();
    }

    pub(super) fn press_right(&mut self, pane: u64, cell: (u16, u16), at: Point<Pixels>) {
        self.moves.borrow_mut().press(Press { pane, grip: Grip::Right, cell, at });
    }

    pub(super) fn drag_pane(&mut self, at: Point<Pixels>, pressed: Option<MouseButton>, cx: &mut Context<Self>) {
        let Some(press) = self.moves.borrow().pressed() else { return };
        if pressed != Some(press.grip.button()) {
            self.cancel_pane_drag(cx);
            return;
        }
        self.moves.borrow_mut().drag(at, &self.layout.borrow(), Instant::now());
        cx.notify();
    }

    pub(super) fn release_pane(&mut self, event: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        let pressed = self.moves.borrow().pressed();
        if pressed.is_none_or(|press| press.grip.button() != event.button) {
            return;
        }
        let now = Instant::now();
        let release = {
            let mut moves = self.moves.borrow_mut();
            moves.drag(event.position, &self.layout.borrow(), now);
            moves.release(self.grid.layout.clone(), now)
        };
        match release {
            Release::Click(press) => self.pane_clicked(press, event.position, window, cx),
            Release::Drop(pane, aim) => send_move(pane, aim, cx),
            Release::Home | Release::Nothing => {}
        }
        cx.notify();
    }

    pub(super) fn cancel_pane_drag(&mut self, cx: &mut Context<Self>) -> bool {
        let cancelled = self.moves.borrow_mut().cancel(Instant::now());
        if cancelled {
            cx.notify();
        }
        cancelled
    }

    fn pane_clicked(&mut self, press: Press, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        if press.grip == Grip::Right {
            if matches!(self.hit(at), Some(Hit::Pane(pane, _)) if pane == press.pane) {
                let entries = self.pane_menu(press.pane, press.cell);
                self.open_menu(at, entries, window, cx);
            }
        } else if self.grid.pane(press.pane).is_some_and(|pane| !pane.active) {
            self.focus_item("pane", press.pane);
        }
    }

    pub(super) fn move_pane(&mut self, pane: u64, aim: Aim, cx: &mut Context<Self>) {
        self.moves.borrow_mut().expect(pane, self.grid.layout.clone(), Instant::now());
        send_move(pane, aim, cx);
    }

    fn move_answered(&mut self, result: &anyhow::Result<String>, cx: &mut Context<Self>) {
        if let Err(e) = result {
            self.say(&e.to_string(), cx);
        }
        self.moves.borrow_mut().answered(&Answer::read(result), Instant::now());
        cx.notify();
    }

    pub(super) fn render_handle(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pane = self.moves.borrow().near().filter(|_| self.frame_shown())?;
        let layout = self.layout.borrow();
        let area = moving::handle(layout.card(pane)?.bounds);
        let at = area.origin - layout.at.origin;
        Some(handle(pane, (at, area.size), cx).into_any_element())
    }

    pub(super) fn render_ghost(&self, metrics: &Metrics) -> Option<impl IntoElement + use<>> {
        if !self.moves.borrow().flying() {
            return None;
        }
        let (moves, layout, grid, metrics) =
            (self.moves.clone(), self.layout.clone(), self.grid.clone(), metrics.clone());
        let ghost = canvas(
            |_, _, _| {},
            move |_, (), window, cx| {
                let Some(ghost) = moves.borrow().ghost(&layout.borrow(), Instant::now()) else { return };
                if let Some(pane) = grid.pane(ghost.pane) {
                    render::paint_ghost(pane, &ghost, &metrics, window, cx);
                }
            },
        );
        Some(deferred(ghost.absolute().size_full()).with_priority(1))
    }
}
