use gpui::{Decorations, Tiling, WindowControls};

use super::{GAP, room, top};
use crate::app::frame::Frame;
use crate::app::placement;
use crate::ui::stack::{Fitting, fitting, most};

const LINE: f32 = 22.;

fn frames() -> [Frame; 4] {
    let controls = WindowControls::default();
    [
        Frame::default(),
        Frame::captioned(controls, false, false),
        Frame::of(Decorations::Client { tiling: Tiling::default() }, controls, false),
        Frame::of(Decorations::Server, controls, false),
    ]
}

#[test]
fn at_the_least_window_a_toast_taller_than_the_room_shows_whole_with_the_more_line() {
    for frame in frames() {
        let room = room(frame, placement::LEAST.1);
        let most = most(room, 2, GAP, LINE);
        assert_eq!(fitting(&[most, 40.], GAP, room, LINE), Fitting { shown: 1, more: true });
    }
}

#[test]
fn the_stack_starts_below_the_title_row_on_every_platform() {
    for frame in frames() {
        assert!(top(frame) >= crate::app::TITLEBAR_HEIGHT + GAP);
    }
}
