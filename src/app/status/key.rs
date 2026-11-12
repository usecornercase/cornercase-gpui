use gpui::Rgba;

use crate::app::{Conn, Root};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StatusKey {
    connection: (Rgba, String, Option<String>),
    starting: bool,
    pane: String,
    size: Option<(u16, u16)>,
    store: u64,
    chips: u64,
    menu: bool,
}

pub(crate) struct StatusInputs {
    pub(crate) connection: (Rgba, String, Option<String>),
    pub(crate) starting: bool,
    pub(crate) pane: String,
    pub(crate) size: Option<(u16, u16)>,
    pub(crate) store: u64,
    pub(crate) chips: u64,
    pub(crate) menu: bool,
}

impl StatusKey {
    pub(crate) fn of(inputs: StatusInputs) -> Self {
        Self {
            connection: inputs.connection,
            starting: inputs.starting,
            pane: inputs.pane,
            size: inputs.size,
            store: inputs.store,
            chips: inputs.chips,
            menu: inputs.menu,
        }
    }
}

impl Root {
    pub(crate) fn status_key(&self) -> StatusKey {
        StatusKey::of(StatusInputs {
            connection: self.connection_label(),
            starting: self.conn == Conn::Starting,
            pane: self.active_pane_summary(),
            size: self.shown().size,
            store: self.parts.store_seen,
            chips: self.parts.chips_seen,
            menu: self.side.menu.is_some(),
        })
    }
}

#[cfg(test)]
mod tests;
