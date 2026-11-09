use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::files::{choice_path, themes_dir};

pub(crate) type Seen = BTreeMap<PathBuf, (SystemTime, u64)>;

fn stat(seen: &mut Seen, path: &Path) {
    if let Ok(meta) = std::fs::metadata(path) {
        seen.insert(path.to_path_buf(), (meta.modified().unwrap_or(SystemTime::UNIX_EPOCH), meta.len()));
    }
}

pub(crate) fn snapshot(folder: &Path) -> Seen {
    let mut seen = Seen::new();
    stat(&mut seen, &choice_path(folder));
    let Ok(entries) = std::fs::read_dir(themes_dir(folder)) else { return seen };
    for entry in entries.filter_map(Result::ok) {
        stat(&mut seen, &entry.path());
    }
    seen
}

pub(crate) fn changed(before: &Seen, now: &Seen) -> Vec<PathBuf> {
    let moved = now.iter().filter(|(path, stamp)| before.get(*path) != Some(stamp)).map(|(path, _)| path.clone());
    let gone = before.keys().filter(|path| !now.contains_key(*path)).cloned();
    moved.chain(gone).collect()
}

#[cfg(test)]
mod tests;
