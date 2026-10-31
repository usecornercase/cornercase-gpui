use std::cell::Cell;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use gpui::{
    Animation, AnimationElement, AnimationExt as _, ElementId, IntoElement, Rgba, Styled, Svg, Transformation,
    percentage, pulsating_between, px,
};

pub(crate) const ENTER: Duration = Duration::from_millis(180);
pub(crate) const PULSE: Duration = Duration::from_millis(1400);
pub(crate) const POP: Duration = Duration::from_millis(110);
pub(crate) const LEAVE: Duration = Duration::from_millis(120);
const TURN: Duration = Duration::from_millis(900);
const ORBIT: Duration = Duration::from_millis(1200);
const ORBIT_FRAME: Duration = Duration::from_micros(33_333);

thread_local! {
    static ORBITS: Cell<usize> = const { Cell::new(0) };
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Way {
    In,
    Pop,
    Out,
}

impl Way {
    fn animation(self) -> Animation {
        match self {
            Way::In => Animation::new(ENTER).with_easing(ease_out),
            Way::Pop => Animation::new(POP).with_easing(ease_out),
            Way::Out => Animation::new(LEAVE).with_easing(ease_in),
        }
    }

    pub(crate) fn shown(self, t: f32) -> f32 {
        match self {
            Way::In | Way::Pop => t,
            Way::Out => 1. - t,
        }
    }
}

pub(crate) fn ease_out(t: f32) -> f32 {
    1. - (1. - t).powi(3)
}

pub(crate) fn ease_in(t: f32) -> f32 {
    t * t
}

pub(crate) fn slide<E: Styled + IntoElement + 'static>(
    element: E,
    id: impl Into<ElementId>,
    way: Way,
    (x, y): (f32, f32),
) -> AnimationElement<E> {
    element.with_animation(id, way.animation(), move |element, t| {
        let shown = way.shown(t);
        let away = 1. - shown;
        element.relative().left(px(x * away)).top(px(y * away)).opacity(shown)
    })
}

pub(crate) fn fade<E: Styled + IntoElement + 'static>(
    element: E,
    id: impl Into<ElementId>,
    way: Way,
) -> AnimationElement<E> {
    element.with_animation(id, way.animation(), move |element, t| element.opacity(way.shown(t)))
}

pub(crate) fn pulse<E: Styled + IntoElement + 'static>(element: E, id: impl Into<ElementId>) -> AnimationElement<E> {
    let breathing = Animation::new(PULSE).repeat().with_easing(pulsating_between(0.3, 1.));
    element.with_animation(id, breathing, Styled::opacity)
}

pub(crate) fn spin(glyph: Svg, id: impl Into<ElementId>) -> AnimationElement<Svg> {
    glyph.with_animation(id, Animation::new(TURN).repeat(), |glyph, t| {
        glyph.with_transformation(Transformation::rotate(percentage(t)))
    })
}

pub(crate) fn clock() -> Duration {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default()
}

pub(crate) fn orbit_turned(now: Duration) -> f32 {
    let turn = ORBIT.as_micros();
    #[expect(clippy::cast_precision_loss, reason = "both stay below a turn's microseconds")]
    let turned = (now.as_micros() % turn) as f32 / turn as f32;
    turned
}

pub(crate) fn orbit(glyph: Svg) -> Svg {
    ORBITS.with(|drawn| drawn.set(drawn.get() + 1));
    glyph.with_transformation(Transformation::rotate(percentage(orbit_turned(clock()))))
}

pub(crate) fn orbits_drawn() -> usize {
    ORBITS.with(Cell::get)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Next {
    Wait,
    Draw,
    Rest,
}

pub(crate) fn next_frame(since_drawn: Duration, since_seen: Duration, orbits: usize) -> Next {
    if orbits == 0 {
        Next::Rest
    } else if since_drawn + since_seen / 2 >= ORBIT_FRAME {
        Next::Draw
    } else {
        Next::Wait
    }
}

#[derive(Default)]
pub(crate) struct Beat {
    running: Cell<bool>,
    drawn: Cell<Option<Instant>>,
    seen: Cell<Option<Instant>>,
}

impl Beat {
    pub(crate) fn drawing(&self, now: Instant) {
        ORBITS.with(|drawn| drawn.set(0));
        self.drawn.set(Some(now));
        self.seen.set(Some(now));
    }

    pub(crate) fn start(&self) -> bool {
        !self.running.replace(true)
    }

    pub(crate) fn tick(&self, now: Instant, orbits: usize) -> Next {
        let since = |at: &Cell<Option<Instant>>| at.get().map_or(ORBIT_FRAME, |at| now.saturating_duration_since(at));
        let next = next_frame(since(&self.drawn), since(&self.seen), orbits);
        self.seen.set(Some(now));
        if next != Next::Wait {
            self.running.set(false);
        }
        next
    }
}

pub(crate) fn veil<E: Styled + IntoElement + 'static>(
    element: E,
    id: impl Into<ElementId>,
    way: Way,
    colour: Rgba,
) -> AnimationElement<E> {
    element.with_animation(id, way.animation(), move |element, t| {
        element.bg(Rgba { a: colour.a * way.shown(t), ..colour })
    })
}

pub(crate) struct Leaving<T> {
    shown: Option<T>,
    gone: Option<(u64, T)>,
    serial: u64,
}

impl<T> Default for Leaving<T> {
    fn default() -> Self {
        Self { shown: None, gone: None, serial: 0 }
    }
}

impl<T> Leaving<T> {
    pub(crate) fn track(&mut self, now: Option<T>) -> Option<u64> {
        let before = std::mem::replace(&mut self.shown, now);
        if self.shown.is_some() {
            self.gone = None;
            return None;
        }
        let before = before?;
        self.serial += 1;
        self.gone = Some((self.serial, before));
        Some(self.serial)
    }

    pub(crate) fn finish(&mut self, serial: u64) -> bool {
        let done = self.gone.as_ref().is_some_and(|(s, _)| *s == serial);
        if done {
            self.gone = None;
        }
        done
    }

    pub(crate) fn gone(&self) -> Option<(u64, &T)> {
        self.gone.as_ref().map(|(serial, item)| (*serial, item))
    }
}

#[cfg(test)]
mod tests;
