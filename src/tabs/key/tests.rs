use super::{TabsInputs, TabsKey};
use crate::model::machines::LinkState;
use crate::model::{MachinePlace, MachineSeen, ThereWorkspace};
use crate::sidebar::{Look, Renaming, Target};
use crate::test_util::{Change, each_change_moves};

#[derive(Clone, Default)]
struct Inputs {
    session: u64,
    store: u64,
    look: Look,
    renaming: Option<Renaming>,
    place: Option<MachinePlace>,
    machine: bool,
    aside: bool,
}

impl Inputs {
    fn key(&self) -> TabsKey {
        TabsKey::of(&TabsInputs {
            session: self.session,
            store: self.store,
            look: &self.look,
            renaming: self.renaming.as_ref(),
            place: self.place.as_ref(),
            machine: self.machine,
            aside: self.aside,
        })
    }
}

const CHANGES: &[Change<Inputs>] = &[
    ("the session", |i| i.session += 1),
    ("the store", |i| i.store += 1),
    ("the look", |i| i.look.model = !i.look.model),
    ("a tab being renamed", |i| i.renaming = Some(Renaming::Here(Target::Tab(4)))),
    ("the machine's place", |i| {
        let seen = MachineSeen { name: "vps".into(), state: LinkState::Linked, ..MachineSeen::default() };
        i.place = Some(MachinePlace::at(&seen, &ThereWorkspace::default(), None));
    }),
    ("a machine's tab shown", |i| i.machine = true),
    ("the sidebar stepping aside", |i| i.aside = true),
];

#[test]
fn each_input_of_the_tab_bar_moves_its_key() {
    each_change_moves(CHANGES, Inputs::key);
}
