use std::collections::HashSet;
use std::fmt::Write as _;

use serde::Deserialize;

use super::{Session, Workspace};

const TIP_LISTED: usize = 6;

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Shared {
    pub workspace: u64,
    pub count: usize,
    pub files: Vec<String>,
}

impl Shared {
    pub(crate) fn beyond(&self, listed: usize) -> usize {
        self.count.saturating_sub(listed.min(self.files.len()))
    }

    fn lists(&self, path: &str, old_path: Option<&str>) -> bool {
        self.files.iter().any(|file| file == path || Some(file.as_str()) == old_path)
    }
}

impl Workspace {
    pub(crate) fn shared_count(&self) -> usize {
        let listed: HashSet<&str> = self.shared.iter().flat_map(|s| &s.files).map(String::as_str).collect();
        let most = self.shared.iter().map(|s| s.count).max().unwrap_or(0);
        listed.len().max(most)
    }
}

impl Session {
    pub(crate) fn workspace_name(&self, workspace: u64) -> String {
        self.workspace(workspace).map_or_else(|| format!("workspace {workspace}"), |(_, w)| w.name.clone())
    }

    pub(crate) fn shared_tip(&self, ws: &Workspace) -> String {
        let mut tip = String::new();
        for shared in &ws.shared {
            let files = if shared.count == 1 { "1 file".to_string() } else { format!("{} files", shared.count) };
            let _ = writeln!(tip, "With {}: {files}", self.workspace_name(shared.workspace));
            for file in shared.files.iter().take(TIP_LISTED) {
                let _ = writeln!(tip, "  {file}");
            }
            if shared.beyond(TIP_LISTED) > 0 {
                let _ = writeln!(tip, "  … and {} more", shared.beyond(TIP_LISTED));
            }
        }
        tip.push_str("Click for the merge check");
        tip
    }

    pub(crate) fn file_shared_tip(&self, workspace: u64, path: &str, old_path: Option<&str>) -> Option<String> {
        let (_, ws) = self.workspace(workspace)?;
        let names: Vec<String> =
            ws.shared.iter().filter(|s| s.lists(path, old_path)).map(|s| self.workspace_name(s.workspace)).collect();
        (!names.is_empty()).then(|| format!("Also changed in {}", names.join(", ")))
    }
}

#[cfg(test)]
mod tests;
