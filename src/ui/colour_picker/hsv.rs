use gpui::Rgba;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Hsv {
    pub h: f32,
    pub s: f32,
    pub v: f32,
}

impl Hsv {
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "a sector of the hue wheel, 0 to 5")]
    pub(crate) fn to_rgba(self, alpha: f32) -> Rgba {
        let wheel = self.h.rem_euclid(1.) * 6.;
        let (sector, along) = (wheel.floor(), wheel - wheel.floor());
        let low = self.v * (1. - self.s);
        let falling = self.v * (1. - along * self.s);
        let rising = self.v * (1. - (1. - along) * self.s);
        let top = self.v;
        let (red, green, blue) = match sector as u8 % 6 {
            0 => (top, rising, low),
            1 => (falling, top, low),
            2 => (low, top, rising),
            3 => (low, falling, top),
            4 => (rising, low, top),
            _ => (top, low, falling),
        };
        Rgba { r: red, g: green, b: blue, a: alpha }
    }

    pub(crate) fn from_rgba(colour: Rgba) -> Hsv {
        let max = colour.r.max(colour.g).max(colour.b);
        let min = colour.r.min(colour.g).min(colour.b);
        let delta = max - min;
        let h = if delta <= 0. {
            0.
        } else if (max - colour.r).abs() < f32::EPSILON {
            ((colour.g - colour.b) / delta).rem_euclid(6.)
        } else if (max - colour.g).abs() < f32::EPSILON {
            (colour.b - colour.r) / delta + 2.
        } else {
            (colour.r - colour.g) / delta + 4.
        };
        Hsv { h: h / 6., s: if max <= 0. { 0. } else { delta / max }, v: max }
    }

    pub(crate) fn pure(self) -> Rgba {
        Hsv { s: 1., v: 1., ..self }.to_rgba(1.)
    }
}

#[cfg(test)]
mod tests;
