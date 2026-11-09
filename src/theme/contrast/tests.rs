use super::*;
use crate::theme::colour::from_bytes;
use crate::theme::current::{Presets, fallback};
use crate::theme::presets::PRESETS;
use crate::theme::resolve::resolve;

fn dim_on_dark() -> Vec<(&'static str, Rgba)> {
    vec![("the ground", from_bytes([11, 12, 14]))]
}

#[test]
fn on_brand_over_cobalt_passes() {
    let check = check_of(&fallback(), "on_brand").expect("checked");
    assert!(check.ratio > 4.8 && check.ratio < 4.9, "{}", check.ratio);
    assert!(check.passes());
}

#[test]
fn every_preset_passes_the_contrast_check() {
    let mut failing = Vec::new();
    for (id, _) in PRESETS {
        let theme = resolve(id, &Presets).expect(id).theme;
        for check in checks(&theme).iter().filter(|check| !check.passes()) {
            failing.push(format!(
                "{id}: {} is {:.2} on {}, needs {}",
                check.key, check.ratio, check.surface, check.needed
            ));
        }
    }
    assert!(failing.is_empty(), "{failing:#?}");
}

#[test]
fn fix_lightens_a_dim_text_on_a_dark_ground_until_it_passes() {
    let dim = from_bytes([40, 44, 50]);
    let (fixed, passes) = fix(dim, &dim_on_dark(), 4.5);
    assert!(passes);
    assert!(ratio(fixed, dim_on_dark()[0].1) >= 4.5);
    assert!(luminance(fixed) > luminance(dim));
}

#[test]
fn fix_darkens_a_pale_text_on_a_light_ground() {
    let light = vec![("the ground", from_bytes([241, 242, 244]))];
    let (fixed, passes) = fix(from_bytes([200, 205, 210]), &light, 4.5);
    assert!(passes);
    assert!(luminance(fixed) < luminance(from_bytes([200, 205, 210])));
}

#[test]
fn fix_keeps_the_hue() {
    let tinted = from_bytes([60, 40, 90]);
    let (fixed, _) = fix(tinted, &dim_on_dark(), 4.5);
    let (before, after): (Hsla, Hsla) = (tinted.into(), fixed.into());
    assert!((before.h - after.h).abs() < 0.02);
}

#[test]
fn fix_says_when_even_white_fails() {
    let middling = vec![("the ground", from_bytes([128, 128, 128]))];
    let (_, passes) = fix(from_bytes([100, 100, 100]), &middling, 7.);
    assert!(!passes);
}
