use super::*;

fn close(a: Rgba, b: Rgba) -> bool {
    (a.r - b.r).abs() < 1e-3 && (a.g - b.g).abs() < 1e-3 && (a.b - b.b).abs() < 1e-3
}

#[test]
fn rgb_to_hsv_and_back_is_the_same_colour() {
    for (r, g, b) in [(0.0, 0.37, 1.0), (0.9, 0.2, 0.4), (0.5, 0.5, 0.5), (1., 1., 0.), (0.1, 0.8, 0.3)] {
        let colour = Rgba { r, g, b, a: 1. };
        assert!(close(Hsv::from_rgba(colour).to_rgba(1.), colour), "{colour:?}");
    }
}

#[test]
fn the_square_s_corners_are_white_black_and_the_pure_hue() {
    let hue = 0.6;
    assert!(close(Hsv { h: hue, s: 0., v: 1. }.to_rgba(1.), Rgba { r: 1., g: 1., b: 1., a: 1. }));
    assert!(close(Hsv { h: hue, s: 1., v: 0. }.to_rgba(1.), Rgba { r: 0., g: 0., b: 0., a: 1. }));
    assert!(close(Hsv { h: hue, s: 1., v: 1. }.to_rgba(1.), Hsv { h: hue, s: 1., v: 1. }.pure()));
}

#[test]
fn the_hue_strip_wraps_at_both_ends() {
    let (start, end) = (Hsv { h: 0., s: 1., v: 1. }.to_rgba(1.), Hsv { h: 1., s: 1., v: 1. }.to_rgba(1.));
    assert!(close(start, end));
    assert!(close(start, Rgba { r: 1., g: 0., b: 0., a: 1. }));
}
