use std::cell::RefCell;
use std::time::Instant;

use gpui::{
    AnyElement, Context, CursorStyle, DispatchPhase, Entity, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, SharedString, Size,
    StatefulInteractiveElement as _, Styled, Window, canvas, deferred, div, px,
};

use super::pane_menu::move_pane_args;
use crate::app::Root;
use crate::app::machine::MachineTab;
use crate::app::machine::acts::MOVING;
use crate::render::layout::Hit;
use crate::render::moving::{self, Aim, Answer, Grip, Moves, Press, Release};
use crate::render::{self, Metrics};
use crate::sidebar::tip;
use crate::store::{self, failed_there};
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
        let movable = self.takes_input() && self.machine.as_ref().is_none_or(MachineTab::sends_boot);
        let near = if movable { moving::near_top(&self.layout.borrow(), at) } else { None };
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
            moves.release(self.shown().grid.layout.clone(), now)
        };
        match release {
            Release::Click(press) => self.pane_clicked(press, event.position, window, cx),
            Release::Drop(pane, aim) => self.send_move(pane, aim, cx),
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
            self.open_pane_menu(press, at, window, cx);
        } else if self.shown().grid.pane(press.pane).is_some_and(|pane| !pane.active) {
            self.focus_on_shown("pane", press.pane);
        }
    }

    fn open_pane_menu(&mut self, press: Press, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        if self.machine.is_some() {
            self.open_there_pane_menu(press.pane, at, window, cx);
        } else if matches!(self.hit(at), Some(Hit::Pane(pane, _)) if pane == press.pane) {
            let entries = self.pane_menu(press.pane, press.cell, cx);
            self.open_menu(at, entries, window, cx);
        }
    }

    pub(super) fn move_pane(&mut self, pane: u64, aim: Aim, cx: &mut Context<Self>) {
        self.moves.borrow_mut().expect(pane, self.shown().grid.layout.clone(), Instant::now());
        self.send_move(pane, aim, cx);
    }

    fn send_move(&mut self, pane: u64, aim: Aim, cx: &mut Context<Self>) {
        if self.machine.is_some() {
            return self.send_move_there(pane, aim, cx);
        }
        let root = cx.entity().downgrade();
        store::queued(cx, move_pane_args(pane, aim), move |result, cx| {
            let _ = root.update(cx, |this, cx| this.move_answered(&result, None, cx));
        });
    }

    fn send_move_there(&mut self, pane: u64, aim: Aim, cx: &mut Context<Self>) {
        let Some(run) = self.machine.as_ref().map(MachineTab::run) else { return self.move_refused(cx) };
        let refusal = self.machine.as_ref().and_then(|tab| tab.boot_refusal(MOVING));
        if let Some(why) = refusal {
            self.tell(&why, cx);
            return self.move_refused(cx);
        }
        if !self.still_there(&run, cx) {
            return self.move_refused(cx);
        }
        let root = cx.entity().downgrade();
        let machine = run.machine.clone();
        let reply = store::reply(cx, move |result, cx| {
            let _ = root.update(cx, |this, cx| this.move_answered(&result, Some(&machine), cx));
        });
        if let Some(tab) = &self.machine {
            tab.send_on_run(move_pane_args(pane, aim), reply);
        }
    }

    fn move_refused(&mut self, cx: &mut Context<Self>) {
        self.moves.borrow_mut().answered(&Answer::Refused, Instant::now());
        cx.notify();
    }

    fn move_answered(&mut self, result: &anyhow::Result<String>, machine: Option<&str>, cx: &mut Context<Self>) {
        if let Err(e) = result {
            match machine {
                Some(machine) => self.say(&failed_there(machine, e), cx),
                None => self.say(&e.to_string(), cx),
            }
        }
        self.moves.borrow_mut().answered(&Answer::read(result), Instant::now());
        cx.notify();
    }

    pub(super) fn render_handle(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pane = self.moves.borrow().near().filter(|_| self.takes_input())?;
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
            (self.moves.clone(), self.layout.clone(), self.shown().grid.clone(), metrics.clone());
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
