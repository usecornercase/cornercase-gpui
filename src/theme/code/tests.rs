use gpui::Hsla;

use super::*;
use crate::theme::colour::luminance;
use crate::theme::current::{Presets, fallback};
use crate::theme::presets::PRESETS;
use crate::theme::resolve::resolve;

fn preset(id: &str) -> Theme {
    resolve(id, &Presets).expect(id).theme
}

fn short_of(theme: &Theme, panes: f32) -> Vec<String> {
    let grounds = grounds(theme, panes);
    let inks = inks(theme, panes);
    let tokens = (1..=6).map(|index| (format!("palette {index}"), inks.ansi[index], TOKEN));
    let all: Vec<(String, Rgba, f32)> = tokens.chain([("comment".to_string(), inks.comment, COMMENT)]).collect();
    all.into_iter()
        .filter(|(_, ink, needed)| *ink != theme.chrome.text && worst(*ink, &grounds).1 + 1e-3 < *needed)
        .map(|(name, ink, _)| format!("{name} is {:.2} at {panes}", worst(ink, &grounds).1))
        .collect()
}

#[test]
fn every_presets_inks_read_on_the_code_blocks_ground() {
    let mut failing = Vec::new();
    for (id, _) in PRESETS {
        let theme = preset(id);
        for panes in [theme.opacity.panes, 0.3, 1.] {
            failing.extend(short_of(&theme, panes).into_iter().map(|short| format!("{id}: {short}")));
        }
    }
    assert!(failing.is_empty(), "{failing:#?}");
}

#[test]
fn a_colour_that_already_reads_is_the_palettes() {
    let theme = fallback();
    let inks = inks(&theme, theme.opacity.panes);
    for index in 1..=6u8 {
        assert_eq!(inks.ansi[usize::from(index)], from_bytes(theme.terminal.palette(index)), "{index}");
    }
}

#[test]
fn a_colour_that_does_not_is_moved_in_lightness_only() {
    let theme = preset("catppuccin-latte");
    let yellow = from_bytes(theme.terminal.palette(3));
    let grounds = grounds(&theme, theme.opacity.panes);
    assert!(worst(yellow, &grounds).1 < TOKEN);
    let moved = inks(&theme, theme.opacity.panes).ansi[3];
    assert!(luminance(moved) < luminance(yellow));
    let (before, after): (Hsla, Hsla) = (yellow.into(), moved.into());
    assert!((before.h - after.h).abs() < 0.02, "{before:?} {after:?}");
    assert!(worst(moved, &grounds).1 >= TOKEN);
}

#[test]
fn a_colour_no_lightness_saves_becomes_the_text() {
    let mut theme = fallback();
    theme.chrome.glass = from_bytes([0, 0, 0]);
    theme.chrome.panel = from_bytes([255, 255, 255]);
    let inks = inks(&theme, 0.);
    assert_eq!(inks.ansi[4], theme.chrome.text);
}

#[test]
fn the_inks_are_kept_while_the_theme_and_the_opacity_stay() {
    let theme = Arc::new(fallback());
    let mut kept = None;
    let first = kept_inks(&mut kept, theme.clone(), 0.8);
    let held = kept.as_ref().map(|k: &Kept| Arc::as_ptr(&k.theme));
    assert_eq!(kept_inks(&mut kept, theme.clone(), 0.8), first);
    assert_eq!(kept.as_ref().map(|k| Arc::as_ptr(&k.theme)), held);
    assert_eq!(first, inks(&theme, 0.8));
}

#[test]
fn another_theme_or_opacity_makes_the_inks_again() {
    let mut kept = None;
    let dark = Arc::new(fallback());
    kept_inks(&mut kept, dark.clone(), 1.);
    let latte = Arc::new(preset("catppuccin-latte"));
    assert_eq!(kept_inks(&mut kept, latte.clone(), 1.), inks(&latte, 1.));
    assert_eq!(kept_inks(&mut kept, latte.clone(), 0.3), inks(&latte, 0.3));
    assert_ne!(inks(&latte, 1.), inks(&dark, 1.));
}

#[test]
fn only_the_colours_a_kind_takes_are_moved() {
    let theme = preset("catppuccin-latte");
    let inks = inks(&theme, theme.opacity.panes);
    for index in [0u8, 7] {
        assert_eq!(inks.ansi[usize::from(index)], from_bytes(theme.terminal.palette(index)), "{index}");
    }
}
