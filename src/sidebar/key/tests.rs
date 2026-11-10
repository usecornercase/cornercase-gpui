use gpui::SharedString;

use super::{KeyParts, SidebarKey};
use crate::app::part::told;
use crate::model::machines::LinkState;
use crate::model::{MachinePlace, MachineSeen, ThereWorkspace};
use crate::sidebar::arrange::Sizes;
use crate::sidebar::machine_rows::{FoldThere, ThereFolds};
use crate::sidebar::{Arrangement, Look, Renaming, Target};
use crate::ui::split::Split;

#[derive(Clone, Default)]
struct Inputs {
    session: u64,
    store: u64,
    hovered: Option<SharedString>,
    renaming: Option<Renaming>,
    there_folds: ThereFolds,
    look: Look,
    sizes: Sizes,
    widths: (f32, f32),
    place: Option<MachinePlace>,
}

impl Inputs {
    fn key(&self) -> SidebarKey {
        SidebarKey::of(&KeyParts {
            session: self.session,
            store: self.store,
            hovered: self.hovered.as_ref(),
            renaming: self.renaming.as_ref(),
            there_folds: &self.there_folds,
            look: &self.look,
            sizes: &self.sizes,
            widths: self.widths,
            place: self.place.as_ref(),
        })
    }
}

fn seen() -> MachineSeen {
    MachineSeen { name: "vps".into(), state: LinkState::Linked, ..MachineSeen::default() }
}

type Change = (&'static str, fn(&mut Inputs));

fn changes() -> Vec<Change> {
    vec![
        ("the session", |i| i.session += 1),
        ("the store", |i| i.store += 1),
        ("the hovered row", |i| i.hovered = Some("row-3".into())),
        ("the row being renamed", |i| i.renaming = Some(Renaming::Here(Target::Tab(4)))),
        ("a fold on a machine", |i| {
            let (seen, ws) = (seen(), ThereWorkspace::default());
            i.there_folds.toggle(FoldThere::workspace(&seen, &ws));
        }),
        ("the arrangement", |i| i.look.arrangement = Arrangement::SideBySide),
        ("a stack's size", |i| i.sizes.stack = Split::new(240., 90., f32::INFINITY).restored(Some(300.), 900.)),
        ("the sidebar's width", |i| i.widths.0 += 10.),
        ("the pair's width", |i| i.widths.1 += 10.),
        ("the machine's place", |i| i.place = Some(MachinePlace::at(&seen(), &ThereWorkspace::default(), None))),
    ]
}

#[test]
fn each_input_of_the_sidebar_moves_its_key() {
    let base = Inputs::default();
    for (what, change) in changes() {
        let mut moved = base.clone();
        change(&mut moved);
        assert_ne!(moved.key(), base.key(), "{what} did not move the key");
    }
}

#[test]
fn the_same_inputs_give_the_same_key() {
    let inputs = Inputs { hovered: Some("row-1".into()), ..Inputs::default() };
    assert_eq!(inputs.key(), inputs.clone().key());
}

#[test]
fn the_sidebar_redraws_only_when_its_key_moves() {
    let key = Inputs::default().key();
    let moved = Inputs { store: 1, ..Inputs::default() }.key();
    assert!(!told(Some(&key), &key).sidebar);
    assert!(told(Some(&key), &moved).sidebar);
}

#[test]
fn the_first_notification_redraws_the_sidebar() {
    assert!(told(None, &Inputs::default().key()).sidebar);
}
