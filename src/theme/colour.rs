use anyhow::{Result, bail};
use gpui::Rgba;

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "clamped to 0..=255 first")]
pub(crate) fn byte(value: f32) -> u8 {
    (value * 255.).round().clamp(0., 255.) as u8
}

pub(crate) fn parse_hex(text: &str) -> Result<Rgba> {
    let digits = text.strip_prefix('#').unwrap_or("");
    let shaped = matches!(digits.len(), 6 | 8) && digits.chars().all(|c| c.is_ascii_hexdigit());
    if !shaped {
        bail!("{text:?} is not a colour: write #rrggbb or #rrggbbaa");
    }
    let channel = |at: usize| digits.get(at..at + 2).and_then(|pair| u8::from_str_radix(pair, 16).ok());
    let [Some(r), Some(g), Some(b)] = [channel(0), channel(2), channel(4)] else { bail!("{text:?} is not a colour") };
    let a = if digits.len() == 8 { channel(6).unwrap_or(255) } else { 255 };
    Ok(from_bytes_alpha([r, g, b], f32::from(a) / 255.))
}

pub(crate) fn hex(colour: Rgba, with_alpha: bool) -> String {
    let [r, g, b] = rgb_bytes(colour);
    if with_alpha && colour.a < 1. {
        format!("#{r:02x}{g:02x}{b:02x}{:02x}", byte(colour.a))
    } else {
        format!("#{r:02x}{g:02x}{b:02x}")
    }
}

pub(crate) fn rgb_bytes(colour: Rgba) -> [u8; 3] {
    [byte(colour.r), byte(colour.g), byte(colour.b)]
}

pub(crate) fn from_bytes([r, g, b]: [u8; 3]) -> Rgba {
    from_bytes_alpha([r, g, b], 1.)
}

pub(crate) fn from_bytes_alpha([r, g, b]: [u8; 3], a: f32) -> Rgba {
    Rgba { r: f32::from(r) / 255., g: f32::from(g) / 255., b: f32::from(b) / 255., a }
}

pub(crate) fn opaque(colour: Rgba) -> Rgba {
    Rgba { a: 1., ..colour }
}

pub(crate) fn with_alpha(colour: Rgba, a: f32) -> Rgba {
    Rgba { a, ..colour }
}

pub(crate) fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let lerp = |x: f32, y: f32| x + (y - x) * t;
    Rgba { r: lerp(a.r, b.r), g: lerp(a.g, b.g), b: lerp(a.b, b.b), a: lerp(a.a, b.a) }
}

pub(crate) fn over(top: Rgba, below: Rgba) -> Rgba {
    let a = top.a + below.a * (1. - top.a);
    if a <= 0. {
        return Rgba { r: 0., g: 0., b: 0., a: 0. };
    }
    let blend = |t: f32, b: f32| (t * top.a + b * below.a * (1. - top.a)) / a;
    Rgba { r: blend(top.r, below.r), g: blend(top.g, below.g), b: blend(top.b, below.b), a }
}

pub(crate) fn luminance(colour: Rgba) -> f32 {
    let linear = |c: f32| if c <= 0.039_28 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
    0.2126 * linear(colour.r) + 0.7152 * linear(colour.g) + 0.0722 * linear(colour.b)
}

#[cfg(test)]
mod tests;
