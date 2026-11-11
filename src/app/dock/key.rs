use gpui::{App, EntityId};

use crate::app::Root;
use crate::panels::PanelKind;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DockKey {
    shown: Option<PanelKind>,
    store: u64,
    session: u64,
}

pub(crate) struct DockInputs {
    pub(crate) shown: Option<PanelKind>,
    pub(crate) store: u64,
    pub(crate) session: u64,
}

impl DockKey {
    pub(crate) fn of(inputs: &DockInputs) -> Self {
        Self { shown: inputs.shown, store: inputs.store, session: inputs.session }
    }

    pub(crate) fn entered(&self, was: Option<&Self>) -> bool {
        self.shown.is_some() && was.is_none_or(|was| was.shown != self.shown)
    }
}

impl Root {
    pub(crate) fn dock_key(&self, cx: &App) -> DockKey {
        DockKey::of(&DockInputs {
            shown: self.store.read(cx).shown_dock(),
            store: self.parts.store_seen,
            session: self.session.revision(),
        })
    }

    pub(crate) fn dock_view_id(&self, cx: &App) -> Option<EntityId> {
        let kind = self.store.read(cx).shown_dock()?;
        self.panels.get(&kind).map(gpui::AnyView::entity_id)
    }
}

#[cfg(test)]
mod tests;
