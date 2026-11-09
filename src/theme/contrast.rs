use gpui::{Hsla, Rgba};

use super::card_edges;
use super::colour::{luminance, opaque, over};
use super::resolve::Theme;

const ON_BRAND: f32 = 4.5;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Check {
    pub key: &'static str,
    pub ratio: f32,
    pub surface: &'static str,
    pub needed: f32,
}

impl Check {
    pub(crate) fn passes(&self) -> bool {
        self.ratio + 1e-3 >= self.needed
    }
}

pub(crate) fn ratio(a: Rgba, b: Rgba) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

pub(crate) fn surfaces(theme: &Theme) -> Vec<(&'static str, Rgba)> {
    let ground = opaque(theme.chrome.glass);
    let card = over(Rgba { a: theme.opacity.cards, ..theme.chrome.surface }, ground);
    let menu = over(card_edges(theme).0, ground);
    vec![
        ("the ground", ground),
        ("cards", card),
        ("panels", over(theme.chrome.panel, ground)),
        ("menus and dialogs", menu),
    ]
}

fn worst(colour: Rgba, surfaces: &[(&'static str, Rgba)]) -> (&'static str, f32) {
    surfaces
        .iter()
        .map(|(name, surface)| (*name, ratio(over(colour, *surface), *surface)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap_or(("the ground", 21.))
}

pub(crate) fn checks(theme: &Theme) -> Vec<Check> {
    let surfaces = surfaces(theme);
    let texts = super::tokens::TOKENS.iter().filter_map(|token| Some((token.key, token.contrast?)));
    let mut checks: Vec<Check> = texts
        .filter_map(|(key, needed)| {
            let (surface, ratio) = worst(theme.chrome.get(key)?, &surfaces);
            Some(Check { key, ratio, surface, needed })
        })
        .collect();
    let brand = opaque(theme.chrome.brand);
    let ratio = ratio(over(theme.chrome.on_brand, brand), brand);
    checks.push(Check { key: "on_brand", ratio, surface: "the brand colour", needed: ON_BRAND });
    checks
}

pub(crate) fn check_of(theme: &Theme, key: &str) -> Option<Check> {
    checks(theme).into_iter().find(|check| check.key == key)
}

pub(crate) fn fix(colour: Rgba, over_what: &[(&'static str, Rgba)], needed: f32) -> (Rgba, bool) {
    let mean =
        over_what.iter().map(|(_, s)| luminance(*s)).sum::<f32>() / over_what.iter().map(|_| 1.).sum::<f32>().max(1.);
    let towards = if luminance(colour) > mean { 1. } else { 0. };
    let start: Hsla = opaque(colour).into();
    let at = |l: f32| Rgba::from(Hsla { l, ..start });
    let passes = |l: f32| worst(at(l), over_what).1 >= needed;
    if !passes(towards) {
        return (at(towards), false);
    }
    let (mut near, mut far) = (start.l, towards);
    for _ in 0..24 {
        let middle = f32::midpoint(near, far);
        if passes(middle) {
            far = middle;
        } else {
            near = middle;
        }
    }
    (at(far), true)
}

pub(crate) fn fix_on_brand(theme: &Theme) -> (Rgba, bool) {
    let brand = opaque(theme.chrome.brand);
    fix(theme.chrome.on_brand, &[("the brand colour", brand)], ON_BRAND)
}

pub(crate) fn fix_token(theme: &Theme, key: &str) -> Option<(Rgba, bool)> {
    if key == "on_brand" {
        return Some(fix_on_brand(theme));
    }
    let needed = super::tokens::TOKENS.iter().find(|token| token.key == key)?.contrast?;
    Some(fix(theme.chrome.get(key)?, &surfaces(theme), needed))
}

#[cfg(test)]
mod tests;
