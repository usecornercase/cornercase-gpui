use gpui::{
    CursorStyle, Decorations, MouseButton, ResizeEdge, Tiling, WindowBackgroundAppearance, WindowBounds,
    WindowControlArea, WindowControls, WindowDecorations,
};

use super::{
    Control, Frame, Grip, Press, RESTORE_POLL, RESTORE_POLL_LATER, Restore, background, cursor, decorations, fitted,
    min_size, outer, press, restore_poll, restore_step, windowed_size,
};
use crate::platform::Os;

fn client(tiling: Tiling) -> Frame {
    Frame::of(Decorations::Client { tiling }, WindowControls::default(), false)
}

fn server() -> Frame {
    Frame::of(Decorations::Server, WindowControls::default(), false)
}

fn windows(maximised: bool, full_screen: bool) -> Frame {
    Frame::captioned(WindowControls::default(), maximised, full_screen)
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
fn a_held_press_starts_one_move_once_the_pointer_drags() {
    let mut grip = Grip { held: true, claimed: false };
    assert!(!grip.take_move(false));
    assert!(grip.take_move(true));
    assert!(!grip.take_move(true));
}

#[test]
fn a_drag_without_a_held_press_moves_nothing() {
    assert!(!Grip::default().take_move(true));
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

fn bounds(width: f32, height: f32) -> gpui::Bounds<gpui::Pixels> {
    gpui::Bounds::new(gpui::point(gpui::px(0.), gpui::px(0.)), gpui::size(gpui::px(width), gpui::px(height)))
}

#[test]
fn leaving_full_screen_on_linux_remembers_the_windowed_size() {
    let windowed = bounds(1304., 824.);
    assert_eq!(windowed_size(Os::Linux, WindowBounds::Fullscreen(windowed)), Some(windowed.size));
    assert_eq!(windowed_size(Os::Linux, WindowBounds::Windowed(windowed)), None);
    assert_eq!(windowed_size(Os::Linux, WindowBounds::Maximized(windowed)), None);
}

#[test]
fn a_mac_leaves_full_screen_as_before() {
    assert_eq!(windowed_size(Os::Mac, WindowBounds::Fullscreen(bounds(1280., 800.))), None);
}

#[test]
fn the_windowed_size_comes_back_once_the_compositor_has_left_full_screen() {
    let (screen, windowed) = (bounds(1500., 950.).size, bounds(1304., 824.).size);
    let floating = client(Tiling::default());
    assert_eq!(restore_step(true, client(Tiling::tiled()), screen, windowed), Restore::Wait);
    assert_eq!(restore_step(false, floating, screen, windowed), Restore::Resize);
    assert_eq!(restore_step(false, floating, windowed, windowed), Restore::Done);
}

#[test]
fn the_restore_keeps_waiting_for_a_slow_compositor_less_often() {
    assert_eq!(restore_poll(0), RESTORE_POLL);
    assert_eq!(restore_poll(99), RESTORE_POLL);
    assert_eq!(restore_poll(100), RESTORE_POLL_LATER);
    assert_eq!(restore_poll(u32::MAX), RESTORE_POLL_LATER);
    assert!(RESTORE_POLL_LATER > RESTORE_POLL);
}

#[test]
fn a_window_that_leaves_full_screen_maximised_or_tiled_keeps_its_size() {
    let (screen, windowed) = (bounds(1500., 950.).size, bounds(1304., 824.).size);
    let maximised = Frame::of(Decorations::Client { tiling: Tiling::tiled() }, WindowControls::default(), true);
    assert_eq!(restore_step(false, maximised, screen, windowed), Restore::Done);
    let tiled = client(Tiling { left: true, top: true, bottom: true, right: false });
    assert_eq!(restore_step(false, tiled, screen, windowed), Restore::Done);
}

#[test]
fn a_linux_window_opened_for_a_margin_the_window_manager_draws_instead_shrinks_by_it() {
    let opened = outer(Os::Linux, (1280., 800.));
    assert_eq!(fitted(Os::Linux, false, server(), false, opened), Some(bounds(1280., 800.).size));
}

#[test]
fn a_window_with_its_own_frame_or_on_a_mac_keeps_its_size() {
    let opened = outer(Os::Linux, (1280., 800.));
    assert_eq!(fitted(Os::Linux, false, client(Tiling::default()), false, opened), None);
    assert_eq!(fitted(Os::Mac, false, Frame::default(), false, bounds(1280., 800.).size), None);
}

#[test]
fn a_maximised_or_full_screen_window_is_left_to_the_window_manager() {
    let opened = outer(Os::Linux, (1280., 800.));
    let maximised = Frame::of(Decorations::Server, WindowControls::default(), true);
    assert_eq!(fitted(Os::Linux, false, maximised, false, opened), None);
    assert_eq!(fitted(Os::Linux, false, server(), true, opened), None);
}

#[test]
fn a_wayland_window_is_never_fitted_whatever_the_compositor_first_answers() {
    let opened = outer(Os::Linux, (1280., 800.));
    assert_eq!(fitted(Os::Linux, true, server(), false, opened), None);
}

#[test]
fn the_minimum_size_counts_the_margin_only_where_the_window_manager_measures_it() {
    let content = bounds(640., 360.).size;
    assert_eq!(min_size(Os::Mac, false, (640., 360.)), content);
    assert_eq!(min_size(Os::Linux, true, (640., 360.)), content);
    assert_eq!(min_size(Os::Linux, false, (640., 360.)), outer(Os::Linux, (640., 360.)));
}

#[test]
fn windows_keeps_the_systems_frame_over_an_opaque_ground() {
    assert_eq!(decorations(Os::Windows), None);
    assert_eq!(background(Os::Windows), WindowBackgroundAppearance::Opaque);
}

#[test]
fn a_windows_caption_draws_no_margin_corner_shadow_or_grip_of_its_own() {
    let frame = windows(false, false);
    assert!(!frame.drawn);
    assert_eq!(frame.margins(), super::Sides { top: 0., right: 0., bottom: 0., left: 0. });
    assert_eq!(frame.corners(), [0.; 4]);
    assert_eq!(frame.grips(), Vec::<ResizeEdge>::new());
    assert!(!frame.floating());
}

#[test]
fn a_windows_caption_offers_minimise_maximise_and_close_and_restore_once_maximised() {
    assert_eq!(windows(false, false).buttons(), vec![Control::Minimise, Control::Maximise, Control::Close]);
    assert_eq!(windows(true, false).buttons(), vec![Control::Minimise, Control::Restore, Control::Close]);
}

#[test]
fn a_full_screen_windows_window_shows_no_caption() {
    let frame = windows(false, true);
    assert_eq!(frame.buttons(), Vec::<Control>::new());
    assert!(frame.resize_band().abs() < f32::EPSILON);
    assert!((frame.title_end() - super::GAP).abs() < f32::EPSILON);
}

#[test]
fn the_caption_buttons_answer_the_systems_hit_test_as_its_own_buttons() {
    assert_eq!(Control::Minimise.area(), WindowControlArea::Min);
    assert_eq!(Control::Maximise.area(), WindowControlArea::Max);
    assert_eq!(Control::Restore.area(), WindowControlArea::Max);
    assert_eq!(Control::Close.area(), WindowControlArea::Close);
}

#[test]
fn the_caption_buttons_are_as_wide_as_windows_draws_them_and_fill_the_title_row() {
    assert_eq!(windows(false, false).button_size(), (46., super::TITLEBAR_HEIGHT));
    assert_eq!(client(Tiling::default()).button_size(), (super::CONTROL, super::CONTROL));
}

#[test]
fn the_caption_buttons_reach_the_windows_right_edge() {
    assert!(windows(false, false).title_end().abs() < f32::EPSILON);
    assert!((client(Tiling::default()).title_end() - super::GAP).abs() < f32::EPSILON);
    assert!((Frame::default().title_end() - super::GAP).abs() < f32::EPSILON);
}

#[test]
fn close_turns_red_with_a_white_glyph_and_the_others_take_a_faint_fill() {
    assert_eq!(Control::Close.caption_hover(), (gpui::rgb(0xc4_2b_1c), gpui::rgb(0xff_ff_ff)));
    let (fill, _) = Control::Minimise.caption_hover();
    assert!(fill.a < 0.1);
    assert_eq!(Control::Maximise.caption_hover(), Control::Restore.caption_hover());
}

#[test]
fn a_windowed_windows_window_keeps_a_band_at_its_top_edge_to_resize_by() {
    assert!(windows(false, false).resize_band() > 0.);
    assert!(windows(true, false).resize_band().abs() < f32::EPSILON);
    assert!(client(Tiling::default()).resize_band().abs() < f32::EPSILON);
    assert!(Frame::default().resize_band().abs() < f32::EPSILON);
}

#[test]
fn a_windows_window_opens_at_the_size_it_shows_and_is_never_fitted_or_restored_by_hand() {
    let shown = bounds(1280., 800.).size;
    assert_eq!(outer(Os::Windows, (1280., 800.)), shown);
    assert_eq!(min_size(Os::Windows, false, (640., 360.)), bounds(640., 360.).size);
    assert_eq!(fitted(Os::Windows, false, windows(false, false), false, shown), None);
    assert_eq!(windowed_size(Os::Windows, WindowBounds::Fullscreen(bounds(1280., 800.))), None);
}
