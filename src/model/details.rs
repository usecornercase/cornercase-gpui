use super::{Session, Tab, Workspace, agent_tree, needs_you};

const MB: u64 = 1024 * 1024;
const GB: u64 = 1024 * MB;

pub(crate) fn memory_size(bytes: u64) -> String {
    let megabytes = bytes.max(MB / 2).saturating_add(MB / 2) / MB;
    if megabytes < 1024 {
        return format!("{megabytes} MB");
    }
    let tenths = bytes.saturating_mul(10).saturating_add(GB / 2) / GB;
    format!("{}.{} GB", tenths / 10, tenths % 10)
}

impl Tab {
    pub(crate) fn memory(&self) -> Option<u64> {
        self.panes.iter().filter_map(|p| p.memory).reduce(u64::saturating_add)
    }
}

impl Workspace {
    pub(crate) fn changed_files(&self) -> u64 {
        self.changes.as_ref().map_or(0, |c| c.files)
    }
}

impl Session {
    pub(crate) fn project_order(&self) -> Vec<u64> {
        self.sections().into_iter().flat_map(|(_, ids)| ids).collect()
    }

    pub(crate) fn agent_needing_you(&self, here: Option<u64>) -> Option<u64> {
        let rows = agent_tree(&self.placed_agents(&self.project_order()), true);
        let start = rows.iter().position(|row| Some(row.pane.id) == here).map_or(0, |at| at + 1);
        let mut in_turn = (0..rows.len()).map(|k| rows[(start + k) % rows.len()].pane);
        in_turn.find(|p| p.status.as_deref().is_some_and(needs_you)).map(|p| p.id)
    }
}

#[cfg(test)]
mod tests;
