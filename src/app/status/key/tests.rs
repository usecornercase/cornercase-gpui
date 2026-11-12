use gpui::rgb;

use super::{StatusInputs, StatusKey};
use crate::test_util::{Change, each_change_moves};

#[derive(Clone, Default)]
struct Inputs {
    dot: u32,
    label: String,
    why: Option<String>,
    starting: bool,
    pane: String,
    size: Option<(u16, u16)>,
    store: u64,
    chips: u64,
    menu: bool,
}

impl Inputs {
    fn key(&self) -> StatusKey {
        StatusKey::of(StatusInputs {
            connection: (rgb(self.dot), self.label.clone(), self.why.clone()),
            starting: self.starting,
            pane: self.pane.clone(),
            size: self.size,
            store: self.store,
            chips: self.chips,
            menu: self.menu,
        })
    }
}

const CHANGES: &[Change<Inputs>] = &[
    ("the connection's words", |i| i.label = "disconnected".into()),
    ("the connection's dot", |i| i.dot = 0x00ff_0000),
    ("the connection's reason", |i| i.why = Some("no server".into())),
    ("connecting", |i| i.starting = true),
    ("the active pane's line", |i| i.pane = "claude · ~/shop".into()),
    ("the size", |i| i.size = Some((80, 24))),
    ("the store", |i| i.store += 1),
    ("the chips", |i| i.chips += 1),
    ("a menu opening", |i| i.menu = true),
];

#[test]
fn each_input_of_the_status_bar_moves_its_key() {
    each_change_moves(CHANGES, Inputs::key);
}
