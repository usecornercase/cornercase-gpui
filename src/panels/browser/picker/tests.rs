use serde_json::json;

use super::*;
use crate::panels::browser::element::Rect;

fn at(rect: Rect, page: Rect) -> Element {
    Element { tag: "div".into(), rect, page, ..Element::default() }
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect { x, y, width, height }
}

#[test]
fn an_element_in_view_is_cropped_with_some_room_without_growing_the_page() {
    let params = crop(&at(rect(100., 50., 200., 40.), rect(0., 300., 800., 600.))).expect("a crop");
    assert_eq!(params["clip"], json!({ "x": 92., "y": 342., "width": 216., "height": 56., "scale": 1 }));
    assert_eq!(params["captureBeyondViewport"], json!(false));
}

#[test]
fn an_element_below_the_fold_is_captured_beyond_the_viewport() {
    let params = crop(&at(rect(10., 580., 100., 100.), rect(0., 0., 800., 600.))).expect("a crop");
    assert_eq!(params["captureBeyondViewport"], json!(true));
}

#[test]
fn the_crop_stops_at_the_top_of_the_document() {
    let params = crop(&at(rect(2., 3., 50., 20.), rect(0., 0., 800., 600.))).expect("a crop");
    assert_eq!(params["clip"]["x"], json!(0.));
    assert_eq!(params["clip"]["y"], json!(0.));
    assert_eq!(params["clip"]["width"], json!(60.));
}

#[test]
fn a_giant_element_is_cropped_to_a_sane_size() {
    let params = crop(&at(rect(0., 0., 5000., 9000.), rect(0., 0., 800., 600.))).expect("a crop");
    assert_eq!(params["clip"]["width"], json!(2000.));
    assert_eq!(params["clip"]["height"], json!(2000.));
}

#[test]
fn a_hidden_element_has_no_crop() {
    assert_eq!(crop(&at(rect(10., 10., 0., 30.), rect(0., 0., 800., 600.))), None);
}

#[test]
fn the_highlight_is_cobalt_with_the_inspectors_tooltip() {
    let config = highlight();
    assert_eq!(config["showInfo"], json!(true));
    assert_eq!(config["contentColor"]["r"], json!(0));
    assert_eq!(config["contentColor"]["g"], json!(95));
    assert_eq!(config["contentColor"]["b"], json!(255));
}

#[test]
fn only_a_pick_in_progress_is_inspecting() {
    assert_eq!(Pick::Picking("s".into()).inspecting(), Some("s"));
    let picked = Pick::Picked(Box::new(Picked { element: Element::default(), crop: Crop::Missing }));
    assert_eq!(picked.inspecting(), None);
}
