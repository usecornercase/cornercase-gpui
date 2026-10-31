use std::io::Cursor;

use image::{ImageFormat, Rgba, RgbaImage};
use serde_json::json;

use super::*;

fn png(colour: [u8; 4]) -> Vec<u8> {
    let mut bytes = Vec::new();
    RgbaImage::from_pixel(2, 1, Rgba(colour)).write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png).expect("png");
    bytes
}

#[test]
fn a_frame_turns_into_the_bgra_gpui_draws() {
    let image = bgra(&png([255, 10, 0, 255])).expect("decodes");
    assert_eq!(image.as_bytes(0).expect("one frame")[..4], [0, 10, 255, 255]);
}

#[test]
fn a_screencast_frame_brings_its_viewport_size() {
    let data = base64::engine::general_purpose::STANDARD.encode(png([0, 0, 0, 255]));
    let params = json!({
        "data": data,
        "sessionId": 3,
        "metadata": { "deviceWidth": 800.0, "deviceHeight": 600.0, "offsetTop": 0.0, "pageScaleFactor": 1.0 }
    });
    let shot = decode(&params).expect("decodes");
    assert_eq!(shot.meta, Meta { width: 800., height: 600., offset_top: 0., scale: 1. });
}

#[test]
fn a_frame_without_metadata_is_refused() {
    let data = base64::engine::general_purpose::STANDARD.encode(png([0, 0, 0, 255]));
    assert!(decode(&json!({ "data": data })).is_err());
}
