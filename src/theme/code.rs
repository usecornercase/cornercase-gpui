use std::ops::RangeInclusive;
use std::sync::Arc;

use gpui::Rgba;

use super::colour::{from_bytes, over};
use super::contrast::{fix, surfaces, worst};
use super::resolve::Theme;

pub(crate) const TOKEN: f32 = 4.5;
pub(crate) const COMMENT: f32 = 3.;
const USED: RangeInclusive<u8> = 1..=6;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Inks {
    pub ansi: [Rgba; 8],
    pub comment: Rgba,
}

pub(crate) struct Kept {
    theme: Arc<Theme>,
    panes: u32,
    inks: Inks,
}

pub(crate) fn grounds(theme: &Theme, panes: f32) -> Vec<(&'static str, Rgba)> {
    let block = Rgba { a: panes, ..from_bytes(theme.terminal.background) };
    surfaces(theme).into_iter().map(|(name, surface)| (name, over(block, surface))).collect()
}

pub(crate) fn inks(theme: &Theme, panes: f32) -> Inks {
    let grounds = grounds(theme, panes);
    let ink = |colour: Rgba, needed: f32| readable(colour, &grounds, needed).unwrap_or(theme.chrome.text);
    Inks {
        ansi: [0, 1, 2, 3, 4, 5, 6, 7].map(|index| {
            let colour = from_bytes(theme.terminal.palette(index));
            if USED.contains(&index) { ink(colour, TOKEN) } else { colour }
        }),
        comment: ink(theme.chrome.faint, COMMENT),
    }
}

pub(crate) fn kept_inks(kept: &mut Option<Kept>, theme: Arc<Theme>, panes: f32) -> Inks {
    if let Some(found) = kept.as_ref().filter(|k| Arc::ptr_eq(&k.theme, &theme) && k.panes == panes.to_bits()) {
        return found.inks;
    }
    let inks = inks(&theme, panes);
    *kept = Some(Kept { theme, panes: panes.to_bits(), inks });
    inks
}

fn readable(colour: Rgba, grounds: &[(&'static str, Rgba)], needed: f32) -> Option<Rgba> {
    if worst(colour, grounds).1 >= needed {
        return Some(colour);
    }
    let (fixed, passes) = fix(colour, grounds, needed);
    passes.then_some(fixed)
}

#[cfg(test)]
mod tests;
