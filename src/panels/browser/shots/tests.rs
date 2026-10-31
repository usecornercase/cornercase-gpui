use serde_json::json;

use super::*;

const PNG_HEADER: [u8; 24] =
    [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n', 0, 0, 0, 13, b'I', b'H', b'D', b'R', 0, 0, 5, 0, 0, 0, 3, 32];

#[test]
fn a_png_tells_its_size_in_its_header() {
    assert_eq!(png_size(&PNG_HEADER), Some((1280, 800)));
}

#[test]
fn something_else_than_a_png_has_no_size() {
    assert_eq!(png_size(b"\xff\xd8\xff\xe0 a jpeg, not a png at all"), None);
    assert_eq!(png_size(&PNG_HEADER[..20]), None);
}

#[test]
fn the_caption_says_what_was_taken_and_how_big() {
    assert_eq!(
        caption(Shot::Full, "http://localhost:5173/", (1600, 5200)),
        "Screenshot of http://localhost:5173/ (full page, 1600×5200 px) "
    );
}

#[test]
fn a_full_page_is_captured_whole_beyond_the_viewport() {
    let params = full_page(&json!({ "cssContentSize": { "width": 800, "height": 3000 } }), 2.).expect("a capture");
    assert_eq!(params["clip"], json!({ "x": 0, "y": 0, "width": 800., "height": 3000., "scale": 1. }));
    assert_eq!(params["captureBeyondViewport"], json!(true));
}

#[test]
fn a_very_long_page_is_scaled_down_to_what_chrome_can_draw() {
    let params = full_page(&json!({ "cssContentSize": { "width": 800, "height": 50000 } }), 2.).expect("a capture");
    assert_eq!(params["clip"]["height"], json!(20000.));
    assert_eq!(params["clip"]["scale"], json!(0.4));
}

#[test]
fn a_page_without_a_size_is_not_captured() {
    assert_eq!(full_page(&json!({}), 2.), None);
}

#[test]
fn screenshots_go_to_the_apps_own_temporary_folder() {
    assert!(shots_dir().ends_with("cornercase-gpui"));
}
