use crate::app::Root;
use crate::model::MachinePlace;
use crate::panels::PanelKind;
use crate::sidebar::Look;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RailKey {
    store: u64,
    session: u64,
    look: Look,
    menu: bool,
    place: Option<MachinePlace>,
    pointed: Option<PanelKind>,
}

pub(crate) struct RailInputs<'a> {
    pub(crate) store: u64,
    pub(crate) session: u64,
    pub(crate) look: &'a Look,
    pub(crate) menu: bool,
    pub(crate) place: Option<&'a MachinePlace>,
    pub(crate) pointed: Option<PanelKind>,
}

impl RailKey {
    pub(crate) fn of(inputs: &RailInputs<'_>) -> Self {
        Self {
            store: inputs.store,
            session: inputs.session,
            look: inputs.look.clone(),
            menu: inputs.menu,
            place: inputs.place.cloned(),
            pointed: inputs.pointed,
        }
    }
}

impl Root {
    pub(crate) fn rail_key(&self, cx: &mut gpui::App) -> RailKey {
        RailKey::of(&RailInputs {
            store: self.parts.store_seen,
            session: self.session.revision(),
            look: &self.side.look.current,
            menu: self.side.menu.is_some(),
            place: self.side.machine_place.as_ref(),
            pointed: crate::tour::Guide::pointed(cx),
        })
    }
}

#[cfg(test)]
mod tests;
