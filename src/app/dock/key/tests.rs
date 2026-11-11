use super::{DockInputs, DockKey};
use crate::panels::PanelKind;
use crate::test_util::{Change, each_change_moves};

#[derive(Clone, Default)]
struct Inputs {
    shown: Option<PanelKind>,
    store: u64,
    session: u64,
}

impl Inputs {
    fn key(&self) -> DockKey {
        DockKey::of(&DockInputs { shown: self.shown, store: self.store, session: self.session })
    }
}

const CHANGES: &[Change<Inputs>] = &[
    ("the shown dock", |i| i.shown = Some(PanelKind::Changes)),
    ("the store", |i| i.store += 1),
    ("the session", |i| i.session += 1),
];

#[test]
fn each_input_of_the_dock_moves_its_key() {
    each_change_moves(CHANGES, Inputs::key);
}

#[test]
fn a_dock_that_opens_or_changes_is_entering_and_one_that_stays_is_not() {
    let key = |shown, store| DockKey::of(&DockInputs { shown, store, session: 0 });
    let changes = key(Some(PanelKind::Changes), 0);
    assert!(changes.entered(None));
    assert!(key(Some(PanelKind::Todo), 0).entered(Some(&changes)));
    assert!(!key(Some(PanelKind::Changes), 3).entered(Some(&changes)));
    assert!(!key(None, 0).entered(Some(&changes)));
}
