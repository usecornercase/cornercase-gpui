use gpui::{Decorations, Tiling, WindowControls};

use super::{BOTTOM, GAP};
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
        let room = placement::LEAST.1 - (frame.title_bottom() + GAP) - BOTTOM;
        let most = most(room, 2, GAP, LINE);
        assert_eq!(fitting(&[most, 40.], GAP, room, LINE), Fitting { shown: 1, more: true });
    }
}
