use gpui::{
    CursorStyle, Decorations, MouseButton, ResizeEdge, Tiling, WindowBackgroundAppearance, WindowControls,
    WindowDecorations,
};

use super::{Control, Frame, Press, background, cursor, decorations, outer, press};
use crate::platform::Os;

fn client(tiling: Tiling) -> Frame {
    Frame::of(Decorations::Client { tiling }, WindowControls::default(), false)
}

fn server() -> Frame {
    Frame::of(Decorations::Server, WindowControls::default(), false)
}

#[test]
fn a_mac_keeps_its_own_title_bar_and_blur() {
    assert_eq!(decorations(Os::Mac), None);
    assert_eq!(background(Os::Mac), WindowBackgroundAppearance::Blurred);
}

#[test]
fn linux_asks_to_draw_its_own_decorations_over_a_see_through_surface() {
    assert_eq!(decorations(Os::Linux), Some(WindowDecorations::Client));
    assert_eq!(background(Os::Linux), WindowBackgroundAppearance::Transparent);
}

#[test]
fn a_window_the_server_decorates_draws_no_frame_and_no_buttons() {
    let frame = server();
    assert!(!frame.drawn);
    assert_eq!(frame.margins(), super::Sides { top: 0., right: 0., bottom: 0., left: 0. });
    assert_eq!(frame.corners(), [0.; 4]);
    assert_eq!(frame.grips(), Vec::<ResizeEdge>::new());
    assert_eq!(frame.buttons(), Vec::<Control>::new());
    assert!(!frame.floating());
}

#[test]
fn a_floating_window_has_margins_rounded_corners_a_shadow_and_eight_grips() {
    let frame = client(Tiling::default());
    let m = frame.margins();
    assert!(m.top > 0.);
    assert_eq!(m, super::Sides { top: m.top, right: m.top, bottom: m.top, left: m.top });
    assert!(frame.corners().iter().all(|r| *r > 0.));
    assert!(frame.floating());
    assert_eq!(frame.grips().len(), 8);
}

#[test]
fn the_windows_corner_follows_the_cards_inside_it() {
    let [corner, ..] = client(Tiling::default()).corners();
    assert!((corner - (crate::theme::radius::CARD + super::GAP)).abs() < f32::EPSILON);
}

#[test]
fn a_maximised_window_has_no_margins_no_corners_no_shadow_and_no_grips() {
    let frame = client(Tiling::tiled());
    assert_eq!(frame.margins(), super::Sides { top: 0., right: 0., bottom: 0., left: 0. });
    assert_eq!(frame.corners(), [0.; 4]);
    assert!(!frame.floating());
    assert_eq!(frame.grips(), Vec::<ResizeEdge>::new());
}

#[test]
fn a_window_tiled_to_the_left_keeps_its_right_side_free() {
    let frame = client(Tiling { top: true, bottom: true, left: true, right: false });
    let m = frame.margins();
    assert!(m.left.abs() < f32::EPSILON && m.top.abs() < f32::EPSILON && m.right > 0.);
    assert_eq!(frame.grips(), vec![ResizeEdge::Right]);
    assert_eq!(frame.corners(), [0.; 4]);
    assert!(!frame.floating());
}

#[test]
fn a_corner_rounds_and_resizes_only_when_both_of_its_sides_are_free() {
    let frame = client(Tiling { top: true, ..Tiling::default() });
    assert_eq!(frame.corners()[..2], [0., 0.]);
    assert!(frame.corners()[2..].iter().all(|r| *r > 0.));
    let grips = frame.grips();
    assert!(!grips.contains(&ResizeEdge::Top) && !grips.contains(&ResizeEdge::TopLeft));
    assert!(grips.contains(&ResizeEdge::BottomLeft) && grips.contains(&ResizeEdge::Left));
}

#[test]
fn every_grip_shows_the_cursor_of_its_direction() {
    assert_eq!(cursor(ResizeEdge::Top), CursorStyle::ResizeUpDown);
    assert_eq!(cursor(ResizeEdge::Left), CursorStyle::ResizeLeftRight);
    assert_eq!(cursor(ResizeEdge::BottomRight), CursorStyle::ResizeUpLeftDownRight);
    assert_eq!(cursor(ResizeEdge::TopRight), CursorStyle::ResizeUpRightDownLeft);
}

#[test]
fn a_drawn_frame_offers_minimise_maximise_and_close() {
    assert_eq!(client(Tiling::default()).buttons(), vec![Control::Minimise, Control::Maximise, Control::Close]);
}

#[test]
fn a_maximised_window_offers_to_restore() {
    let frame = Frame::of(Decorations::Client { tiling: Tiling::tiled() }, WindowControls::default(), true);
    assert_eq!(frame.buttons(), vec![Control::Minimise, Control::Restore, Control::Close]);
}

#[test]
fn only_what_the_compositor_can_do_gets_a_button_and_close_always_shows() {
    let controls = WindowControls { fullscreen: false, maximize: false, minimize: false, window_menu: false };
    let frame = Frame::of(Decorations::Client { tiling: Tiling::default() }, controls, false);
    assert_eq!(frame.buttons(), vec![Control::Close]);
}

#[test]
fn a_double_press_on_the_title_row_zooms_and_a_single_one_holds_for_a_move() {
    assert_eq!(press(MouseButton::Left, 2, false, true), Press::Zoom);
    assert_eq!(press(MouseButton::Left, 1, false, true), Press::Hold);
}

#[test]
fn a_right_press_opens_the_window_menu_only_where_there_is_one() {
    assert_eq!(press(MouseButton::Right, 1, false, true), Press::Menu);
    assert_eq!(press(MouseButton::Right, 1, false, false), Press::Pass);
    assert_eq!(press(MouseButton::Middle, 1, false, true), Press::Pass);
}

#[test]
fn a_press_a_tab_or_button_took_does_nothing_to_the_window() {
    for (button, clicks) in [(MouseButton::Left, 1), (MouseButton::Left, 2), (MouseButton::Right, 1)] {
        assert_eq!(press(button, clicks, true, true), Press::Pass);
    }
}

#[test]
fn a_linux_window_opens_larger_by_its_margins_so_it_shows_the_size_a_mac_does() {
    assert_eq!(outer(Os::Mac, (1280., 800.)), gpui::size(gpui::px(1280.), gpui::px(800.)));
    let linux = outer(Os::Linux, (1280., 800.));
    let m = client(Tiling::default()).margins();
    assert_eq!(linux, gpui::size(gpui::px(1280. + m.left + m.right), gpui::px(800. + m.top + m.bottom)));
}
