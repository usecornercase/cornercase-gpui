use std::sync::mpsc;
use std::time::Duration;

use super::super::body::image_size;
use super::{Missing, Pixels, png, rgba_of, within};

#[test]
fn a_thumbnail_without_alpha_is_opaque() {
    assert_eq!(rgba_of(&[1, 2, 3, 0, 4, 5, 6, 0]), [3, 2, 1, 255, 6, 5, 4, 255]);
}

#[test]
fn a_thumbnail_with_alpha_loses_its_premultiplication() {
    let bgra = [64, 32, 16, 128, 1, 2, 3, 255, 0, 0, 0, 0];
    assert_eq!(rgba_of(&bgra), [32, 64, 128, 128, 3, 2, 1, 255, 0, 0, 0, 0]);
}

#[test]
fn a_thumbnail_becomes_a_png_of_its_size() {
    let pixels = Pixels { width: 2, height: 1, bgra: vec![0, 0, 255, 255, 0, 255, 0, 255] };
    let png = png(&pixels).expect("a png");
    assert_eq!(image_size(&png), Some((2., 1.)));
}

#[test]
fn a_thumbnail_of_the_wrong_size_is_refused() {
    let pixels = Pixels { width: 2, height: 2, bgra: vec![0; 4] };
    assert!(matches!(png(&pixels), Err(Missing::Failed(_))));
}

#[test]
fn an_answer_in_time_is_kept() {
    assert_eq!(within(Duration::from_secs(5), || Ok(7)), Ok(7));
    assert_eq!(within::<u8>(Duration::from_secs(5), || Err(Missing::NoReader)), Err(Missing::NoReader));
}

#[test]
fn an_answer_too_late_is_a_failure_that_says_so() {
    let (hold, wait) = mpsc::channel::<()>();
    let late = within(Duration::ZERO, move || {
        let _ = wait.recv();
        Ok(1)
    });
    assert_eq!(late, Err(Missing::Failed("Windows made no thumbnail within 0 s".into())));
    drop(hold);
}
