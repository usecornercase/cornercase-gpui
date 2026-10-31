use gpui::{
    Bounds, Context, DispatchPhase, FocusHandle, Hitbox, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, NavigationDirection, Pixels, Point, ScrollDelta, ScrollWheelEvent, WeakEntity, Window, px,
};
use serde_json::{Value, json};

use super::Browser;
use super::frame::Meta;
use super::keyboard::modifiers;
use super::viewport::{page_point, to_page};

const LINE: f32 = 40.;
const AWAY: (f32, f32) = (-1., -1.);

#[derive(Default)]
struct Flight {
    busy: bool,
    queued: Option<Value>,
}

#[derive(Default)]
pub(crate) struct Pointer {
    buttons: u8,
    button: Option<MouseButton>,
    clicks: usize,
    inside: bool,
    moves: Flight,
    wheels: Flight,
}

impl Pointer {
    pub(crate) fn dragging(&self) -> bool {
        self.buttons != 0
    }

    fn flight(&mut self, wheel: bool) -> &mut Flight {
        if wheel { &mut self.wheels } else { &mut self.moves }
    }
}

pub(crate) struct Mouse {
    pub(crate) kind: &'static str,
    pub(crate) at: (f32, f32),
    pub(crate) button: Option<MouseButton>,
    pub(crate) buttons: u8,
    pub(crate) clicks: usize,
    pub(crate) modifiers: Modifiers,
}

pub(crate) fn button(button: MouseButton) -> (&'static str, u8) {
    match button {
        MouseButton::Left => ("left", 1),
        MouseButton::Right => ("right", 2),
        MouseButton::Middle => ("middle", 4),
        MouseButton::Navigate(NavigationDirection::Back) => ("back", 8),
        MouseButton::Navigate(NavigationDirection::Forward) => ("forward", 16),
    }
}

pub(crate) fn mouse(m: &Mouse) -> Value {
    json!({
        "type": m.kind,
        "x": m.at.0,
        "y": m.at.1,
        "button": m.button.map_or("none", |b| button(b).0),
        "buttons": m.buttons,
        "clickCount": m.clicks,
        "modifiers": modifiers(m.modifiers),
        "pointerType": "mouse",
    })
}

pub(crate) fn wheel(at: (f32, f32), delta: ScrollDelta, held: Modifiers) -> Value {
    let delta = delta.pixel_delta(px(LINE));
    json!({
        "type": "mouseWheel",
        "x": at.0,
        "y": at.1,
        "deltaX": -f32::from(delta.x),
        "deltaY": -f32::from(delta.y),
        "modifiers": modifiers(held),
        "pointerType": "mouse",
    })
}

pub(crate) fn merged(queued: Option<Value>, mut next: Value) -> Value {
    let Some(queued) = queued else { return next };
    for axis in ["deltaX", "deltaY"] {
        let sum = queued[axis].as_f64().unwrap_or(0.) + next[axis].as_f64().unwrap_or(0.);
        next[axis] = json!(sum);
    }
    next
}

#[derive(Clone)]
pub(crate) struct Area {
    pub(crate) entity: WeakEntity<Browser>,
    pub(crate) focus: FocusHandle,
    pub(crate) hitbox: Hitbox,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) meta: Meta,
    pub(crate) menu_open: bool,
}

impl Area {
    fn inside(&self, at: Point<Pixels>, window: &Window) -> Option<(f32, f32)> {
        if self.hitbox.is_hovered(window) { to_page(at, self.bounds, &self.meta) } else { None }
    }

    fn anywhere(&self, at: Point<Pixels>) -> (f32, f32) {
        page_point(at, self.bounds, &self.meta)
    }
}

pub(crate) fn listen(area: &Area, window: &mut Window) {
    listen_buttons(area, window);
    let a = area.clone();
    window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
        if phase == DispatchPhase::Capture {
            let at = (a.inside(e.position, window), a.anywhere(e.position));
            let _ = a.entity.update(cx, |this, cx| this.moved(e, at, cx));
        }
    });
    let a = area.clone();
    window.on_mouse_event(move |e: &ScrollWheelEvent, phase, window, cx| {
        let at = a.inside(e.position, window).filter(|_| phase == DispatchPhase::Bubble);
        if let Some(at) = at {
            let _ = a.entity.update(cx, |this, cx| this.wheeled(wheel(at, e.delta, e.modifiers), cx));
        }
    });
}

fn listen_buttons(area: &Area, window: &mut Window) {
    let a = area.clone();
    window.on_mouse_event(move |e: &MouseDownEvent, phase, window, cx| {
        let Some(at) = a.inside(e.position, window).filter(|_| phase == DispatchPhase::Bubble) else { return };
        window.focus(&a.focus);
        let menu_open = a.menu_open;
        let _ = a.entity.update(cx, |this, cx| if menu_open { this.close_choice(cx) } else { this.pressed(e, at, cx) });
    });
    let a = area.clone();
    window.on_mouse_event(move |e: &MouseUpEvent, phase, _, cx| {
        if phase == DispatchPhase::Capture {
            let at = a.anywhere(e.position);
            let _ = a.entity.update(cx, |this, cx| this.released(e.button, (at, e.modifiers), cx));
        }
    });
}

impl Browser {
    fn send_mouse(&self, params: Value) {
        if let Some(session) = self.active_session() {
            self.send(Some(&session), "Input.dispatchMouseEvent", params);
        }
    }

    fn fly(&mut self, wheel: bool, params: Value, cx: &mut Context<Self>) {
        let Some(session) = self.active_session() else { return };
        let flight = self.input.pointer.flight(wheel);
        if flight.busy {
            let queued = flight.queued.take();
            flight.queued = Some(if wheel { merged(queued, params) } else { params });
            return;
        }
        flight.busy = true;
        self.call((Some(&session), "Input.dispatchMouseEvent", params), cx, move |this, _, cx| {
            let flight = this.input.pointer.flight(wheel);
            flight.busy = false;
            if let Some(next) = flight.queued.take() {
                this.fly(wheel, next, cx);
            }
        });
    }

    fn flush(&mut self) {
        for wheel in [false, true] {
            if let Some(queued) = self.input.pointer.flight(wheel).queued.take() {
                self.send_mouse(queued);
            }
        }
    }

    fn pressed(&mut self, e: &MouseDownEvent, at: (f32, f32), cx: &mut Context<Self>) {
        self.flush();
        let p = &mut self.input.pointer;
        p.buttons |= button(e.button).1;
        (p.button, p.clicks, p.inside) = (Some(e.button), e.click_count, true);
        let m = Mouse {
            kind: "mousePressed",
            at,
            button: p.button,
            buttons: p.buttons,
            clicks: p.clicks,
            modifiers: e.modifiers,
        };
        self.send_mouse(mouse(&m));
        cx.notify();
    }

    fn released(&mut self, which: MouseButton, (at, held): ((f32, f32), Modifiers), cx: &mut Context<Self>) {
        let bit = button(which).1;
        if self.input.pointer.buttons & bit == 0 {
            return;
        }
        self.flush();
        let p = &mut self.input.pointer;
        p.buttons &= !bit;
        let m = Mouse {
            kind: "mouseReleased",
            at,
            button: Some(which),
            buttons: p.buttons,
            clicks: p.clicks,
            modifiers: held,
        };
        self.send_mouse(mouse(&m));
        cx.notify();
    }

    fn moved(
        &mut self,
        e: &MouseMoveEvent,
        (inside, anywhere): (Option<(f32, f32)>, (f32, f32)),
        cx: &mut Context<Self>,
    ) {
        let p = &mut self.input.pointer;
        if p.dragging() && e.pressed_button.is_none() {
            let which = p.button.unwrap_or(MouseButton::Left);
            self.released(which, (anywhere, e.modifiers), cx);
            return;
        }
        let at = if p.dragging() { Some(anywhere) } else { inside };
        let was_inside = std::mem::replace(&mut p.inside, at.is_some());
        let Some(at) = at.or(was_inside.then_some(AWAY)) else { return };
        let button = p.button.filter(|_| p.dragging());
        let m = Mouse { kind: "mouseMoved", at, button, buttons: p.buttons, clicks: 0, modifiers: e.modifiers };
        self.fly(false, mouse(&m), cx);
    }

    fn wheeled(&mut self, params: Value, cx: &mut Context<Self>) {
        self.fly(true, params, cx);
    }
}

#[cfg(test)]
mod tests;
