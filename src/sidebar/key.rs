use gpui::SharedString;

use super::arrange::Sizes;
use super::machine_rows::ThereFolds;
use super::{Look, Renaming};
use crate::app::Root;
use crate::model::MachinePlace;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SidebarKey {
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

pub(crate) struct KeyParts<'a> {
    pub(crate) session: u64,
    pub(crate) store: u64,
    pub(crate) hovered: Option<&'a SharedString>,
    pub(crate) renaming: Option<&'a Renaming>,
    pub(crate) there_folds: &'a ThereFolds,
    pub(crate) look: &'a Look,
    pub(crate) sizes: &'a Sizes,
    pub(crate) widths: (f32, f32),
    pub(crate) place: Option<&'a MachinePlace>,
}

impl SidebarKey {
    pub(crate) fn of(parts: &KeyParts<'_>) -> Self {
        Self {
            session: parts.session,
            store: parts.store,
            hovered: parts.hovered.cloned(),
            renaming: parts.renaming.cloned(),
            there_folds: parts.there_folds.clone(),
            look: parts.look.clone(),
            sizes: *parts.sizes,
            widths: parts.widths,
            place: parts.place.cloned(),
        }
    }
}

impl Root {
    pub(crate) fn sidebar_key(&self) -> SidebarKey {
        SidebarKey::of(&KeyParts {
            session: self.session.revision(),
            store: self.parts.store_seen,
            hovered: self.side.hovered.as_ref(),
            renaming: self.side.rename.as_ref().map(|rename| &rename.what),
            there_folds: &self.side.there_folds,
            look: &self.side.look.current,
            sizes: &self.side.sizes,
            widths: (self.sidebar_width.size(), self.pair_width.size()),
            place: self.side.machine_place.as_ref(),
        })
    }
}

#[cfg(test)]
mod tests;
