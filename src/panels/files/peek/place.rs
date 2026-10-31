use gpui::{Bounds, Pixels, Size};

use super::super::style::small_f32;

pub(super) const MARGIN: f32 = 10.;
pub(super) const WIDTH: f32 = 660.;
pub(super) const NARROW: f32 = 340.;
const GAP: f32 = 6.;
const BESIDE: f32 = 420.;
const LIFT: f32 = 10.;
const ROOMY: f32 = 260.;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Side {
    Beside,
    Below,
    Above,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Place {
    pub side: Side,
    pub left: f32,
    pub top: Option<f32>,
    pub bottom: Option<f32>,
    pub width: f32,
    pub height: f32,
}

struct Span {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    room_w: f32,
    room_h: f32,
}

impl Span {
    fn new(link: Bounds<Pixels>, room: Size<Pixels>) -> Self {
        Self {
            x: f32::from(link.origin.x),
            y: f32::from(link.origin.y),
            w: f32::from(link.size.width),
            h: f32::from(link.size.height),
            room_w: f32::from(room.width),
            room_h: f32::from(room.height),
        }
    }

    fn beside(&self) -> Place {
        let left = self.x + self.w + GAP;
        let width = WIDTH.min(self.room_w - MARGIN - left);
        let (top, bottom) = if self.y + self.h / 2. <= self.room_h / 2. {
            (Some((self.y - LIFT).max(MARGIN)), None)
        } else {
            (None, Some((self.room_h - self.y - self.h - LIFT).max(MARGIN)))
        };
        let height = self.room_h - top.or(bottom).unwrap_or(MARGIN) - MARGIN;
        Place { side: Side::Beside, left, top, bottom, width, height }
    }

    fn across(&self) -> Place {
        let below = self.room_h - (self.y + self.h) - GAP - MARGIN;
        let above = self.y - GAP - MARGIN;
        let width = WIDTH.min(self.room_w - 2. * MARGIN).max(0.);
        let left = self.x.min(self.room_w - MARGIN - width).max(MARGIN);
        if below >= ROOMY || below >= above {
            let top = Some(self.y + self.h + GAP);
            Place { side: Side::Below, left, top, bottom: None, width, height: below.max(0.) }
        } else {
            let bottom = Some(self.room_h - self.y + GAP);
            Place { side: Side::Above, left, top: None, bottom, width, height: above.max(0.) }
        }
    }
}

pub(super) fn place(link: Bounds<Pixels>, room: Size<Pixels>) -> Place {
    let span = Span::new(link, room);
    if span.room_w - (span.x + span.w) - GAP - MARGIN >= BESIDE { span.beside() } else { span.across() }
}

pub(super) fn fit((w, h): (f32, f32), (max_w, max_h): (f32, f32)) -> (f32, f32) {
    if w <= 0. || h <= 0. {
        return (max_w.max(0.), max_h.max(0.));
    }
    let scale = (max_w / w).min(max_h / h).clamp(0., 1.);
    ((w * scale).round(), (h * scale).round())
}

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "a count of rows, clamped first")]
pub(super) fn shown_rows(lines: usize, height: f32, row: f32, most: usize) -> usize {
    let room = (height / row).floor().clamp(0., small_f32(most)) as usize;
    lines.min(room).max(1)
}

#[cfg(test)]
mod tests;
