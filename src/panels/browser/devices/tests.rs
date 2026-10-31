use serde_json::json;

use super::*;

const DESKTOP: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36";

fn panel(width: f32, height: f32, scale: f32) -> View {
    View { width, height, scale, ..View::default() }
}

fn named(name: &str) -> Device {
    Device::of(PRESETS.iter().copied().find(|p| p.name == name))
}

#[test]
fn responsive_fills_the_panel_at_the_display_scale() {
    let metrics = Device::default().metrics(&panel(720., 600., 2.)).expect("measured");
    assert_eq!(metrics, json!({ "width": 720, "height": 600, "deviceScaleFactor": 2.0, "mobile": false }));
}

#[test]
fn responsive_sends_nothing_before_the_panel_is_measured() {
    assert_eq!(Device::default().metrics(&View::default()), None);
}

#[test]
fn a_phone_is_a_mobile_screen_of_its_own_size_and_density() {
    let metrics = named("iPhone 15").metrics(&panel(720., 600., 2.)).expect("a preset");
    assert_eq!(
        metrics,
        json!({
            "width": 393,
            "height": 852,
            "deviceScaleFactor": 3.0,
            "mobile": true,
            "screenWidth": 393,
            "screenHeight": 852,
            "screenOrientation": { "type": "portraitPrimary", "angle": 0 },
        })
    );
}

#[test]
fn a_preset_is_sized_even_before_the_panel_is_measured() {
    assert!(named("Pixel 8").metrics(&View::default()).is_some());
}

#[test]
fn a_laptop_is_not_a_mobile_screen() {
    let metrics = named("Laptop").metrics(&View::default()).expect("a preset");
    assert_eq!((metrics["mobile"].clone(), metrics["width"].clone()), (json!(false), json!(1280)));
}

#[test]
fn rotating_swaps_the_sides_and_turns_the_screen() {
    let metrics = named("iPhone 15").rotate().metrics(&View::default()).expect("a preset");
    assert_eq!((metrics["width"].clone(), metrics["height"].clone()), (json!(852), json!(393)));
    assert_eq!(metrics["screenOrientation"], json!({ "type": "landscapePrimary", "angle": 90 }));
}

#[test]
fn rotating_twice_comes_back() {
    let phone = named("Pixel 8");
    assert_eq!(phone.rotate().rotate(), phone);
}

#[test]
fn responsive_does_not_rotate() {
    assert!(!Device::default().rotate().rotated);
}

#[test]
fn responsive_frames_are_the_panel_in_device_pixels() {
    assert_eq!(Device::default().frame_pixels(&panel(720., 600., 2.)), (1440, 1200));
}

#[test]
fn a_device_that_fits_is_cast_at_the_display_scale_not_its_own() {
    assert_eq!(named("iPhone 15").frame_pixels(&panel(1000., 2000., 2.)), (786, 1704));
}

#[test]
fn a_device_shrunk_to_fit_is_cast_no_bigger_than_it_is_shown() {
    assert_eq!(named("iPhone 15").frame_pixels(&panel(800., 600., 2.)), (554, 1200));
}

#[test]
fn a_cast_never_exceeds_the_device_density() {
    assert_eq!(named("iPhone 15").frame_pixels(&panel(1000., 2000., 4.)), (1179, 2556));
}

#[test]
fn a_device_cast_before_the_panel_is_measured_takes_a_retina_scale() {
    assert_eq!(named("Laptop").frame_pixels(&View::default()), (2560, 1600));
}

#[test]
fn the_size_label_follows_the_rotation() {
    assert_eq!(named("iPad Air").size_label(&View::default()), "820 × 1180");
    assert_eq!(named("iPad Air").rotate().size_label(&View::default()), "1180 × 820");
}

#[test]
fn responsive_labels_the_panel_size() {
    assert_eq!(Device::default().size_label(&panel(719.6, 600.2, 2.)), "720 × 600");
    assert_eq!(Device::default().size_label(&View::default()), "");
}

#[test]
fn the_density_label_drops_needless_decimals() {
    assert_eq!(named("iPhone 15").scale_label().as_deref(), Some("3×"));
    assert_eq!(named("Pixel 8").scale_label().as_deref(), Some("2.625×"));
    assert_eq!(Device::default().scale_label(), None);
}

#[test]
fn the_version_is_read_from_the_browser_agent() {
    assert_eq!(chrome_version(DESKTOP), Some("141.0.0.0"));
    assert_eq!(chrome_version("curl/8.0"), None);
}

#[test]
fn an_iphone_announces_itself_with_the_browser_version() {
    let agent = named("iPhone 15").user_agent(DESKTOP);
    assert!(agent.contains("(iPhone; CPU iPhone OS"), "{agent}");
    assert!(agent.contains("CriOS/141.0.0.0 Mobile/"), "{agent}");
}

#[test]
fn an_ipad_announces_itself_as_one() {
    assert!(named("iPad Air").user_agent(DESKTOP).contains("(iPad; CPU OS"));
}

#[test]
fn a_pixel_announces_mobile_chrome_on_android() {
    let agent = named("Pixel 8").user_agent(DESKTOP);
    assert!(agent.contains("Android") && agent.contains("Chrome/141.0.0.0 Mobile Safari"), "{agent}");
}

#[test]
fn a_laptop_keeps_the_desktop_agent() {
    assert_eq!(named("Laptop").user_agent(DESKTOP), DESKTOP);
    assert_eq!(Device::default().user_agent(DESKTOP), DESKTOP);
}

#[test]
fn phones_and_tablets_emulate_touch() {
    let calls = named("iPad Air").emulation(Some(DESKTOP));
    let touch = calls.iter().find(|(m, _)| *m == "Emulation.setTouchEmulationEnabled").expect("touch");
    assert_eq!(touch.1, json!({ "enabled": true, "maxTouchPoints": 5 }));
    let mouse = calls.iter().find(|(m, _)| *m == "Emulation.setEmitTouchEventsForMouse").expect("mouse");
    assert_eq!(mouse.1["enabled"], json!(true));
}

#[test]
fn responsive_turns_touch_off_and_restores_the_desktop_agent() {
    let calls = Device::default().emulation(Some(DESKTOP));
    assert!(calls.contains(&("Emulation.setTouchEmulationEnabled", json!({ "enabled": false }))));
    assert!(calls.contains(&("Emulation.setUserAgentOverride", json!({ "userAgent": DESKTOP }))));
}

#[test]
fn no_agent_is_sent_before_the_browser_version_is_known() {
    let calls = named("iPhone 15").emulation(None);
    assert!(calls.iter().all(|(m, _)| *m != "Emulation.setUserAgentOverride"));
}

#[test]
fn only_a_change_of_system_changes_the_agent() {
    assert!(named("Laptop").same_agent(&Device::default()));
    assert!(named("iPhone 15").same_agent(&named("iPhone 15").rotate()));
    assert!(!named("iPhone 15").same_agent(&named("Pixel 8")));
}

#[test]
fn every_preset_has_its_form_icon() {
    let icons: Vec<Icon> = PRESETS.iter().map(Preset::icon).collect();
    assert_eq!(icons, [Icon::Phone, Icon::Phone, Icon::Tablet, Icon::Laptop, Icon::Monitor]);
    assert_eq!(Device::default().icon(), Icon::Devices);
}
