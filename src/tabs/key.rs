use crate::app::Root;
use crate::model::MachinePlace;
use crate::sidebar::{Look, Renaming};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TabsKey {
    session: u64,
    store: u64,
    look: Look,
    renaming: Option<Renaming>,
    place: Option<MachinePlace>,
    machine: bool,
    aside: bool,
}

pub(crate) struct TabsInputs<'a> {
    pub(crate) session: u64,
    pub(crate) store: u64,
    pub(crate) look: &'a Look,
    pub(crate) renaming: Option<&'a Renaming>,
    pub(crate) place: Option<&'a MachinePlace>,
    pub(crate) machine: bool,
    pub(crate) aside: bool,
}

impl TabsKey {
    pub(crate) fn of(inputs: &TabsInputs<'_>) -> Self {
        Self {
            session: inputs.session,
            store: inputs.store,
            look: inputs.look.clone(),
            renaming: inputs.renaming.cloned(),
            place: inputs.place.cloned(),
            machine: inputs.machine,
            aside: inputs.aside,
        }
    }
}

impl Root {
    pub(crate) fn tabs_key(&self) -> TabsKey {
        TabsKey::of(&TabsInputs {
            session: self.session.revision(),
            store: self.parts.store_seen,
            look: &self.side.look.current,
            renaming: self.side.rename.as_ref().map(|rename| &rename.what),
            place: self.side.machine_place.as_ref(),
            machine: self.machine.is_some(),
            aside: self.side.aside,
        })
    }
}

#[cfg(test)]
mod tests;
