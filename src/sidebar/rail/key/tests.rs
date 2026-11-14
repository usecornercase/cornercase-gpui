use super::{RailInputs, RailKey};
use crate::model::MachinePlace;
use crate::model::machines::LinkState;
use crate::model::{MachineSeen, ThereWorkspace};
use crate::panels::PanelKind;
use crate::sidebar::Look;
use crate::test_util::{Change, each_change_moves};

#[derive(Clone, Default)]
struct Inputs {
    store: u64,
    session: u64,
    look: Look,
    menu: bool,
    place: Option<MachinePlace>,
    pointed: Option<PanelKind>,
}

impl Inputs {
    fn key(&self) -> RailKey {
        RailKey::of(&RailInputs {
            store: self.store,
            session: self.session,
            look: &self.look,
            menu: self.menu,
            place: self.place.as_ref(),
            pointed: self.pointed,
        })
    }
}

const CHANGES: &[Change<Inputs>] = &[
    ("the store, which a sheet opening or closing notifies", |i| i.store += 1),
    ("the session", |i| i.session += 1),
    ("the look", |i| i.look.todo_dot = !i.look.todo_dot),
    ("an open menu", |i| i.menu = true),
    ("the machine's place", |i| {
        let seen = MachineSeen { name: "vps".into(), state: LinkState::Linked, ..MachineSeen::default() };
        i.place = Some(MachinePlace::at(&seen, &ThereWorkspace::default(), None));
    }),
    ("the panel a tour points at", |i| i.pointed = Some(PanelKind::Changes)),
];

#[test]
fn each_input_of_the_rail_moves_its_key() {
    each_change_moves(CHANGES, Inputs::key);
}
