use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub(super) const MAX_ENTRIES: usize = 10_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Entry {
    pub name: String,
    pub dir: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Listing {
    pub entries: Vec<Entry>,
    pub truncated: bool,
}

pub(super) fn list(root: &Path, rel: &Path) -> std::io::Result<Listing> {
    let dir = root.join(rel);
    std::fs::metadata(&dir)?;
    let walk = ignore::WalkBuilder::new(&dir)
        .max_depth(Some(1))
        .hidden(false)
        .parents(true)
        .ignore(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .require_git(false)
        .follow_links(false)
        .filter_entry(|e| e.file_name() != ".git")
        .build();
    let mut listing = Listing::default();
    for entry in walk.flatten() {
        if entry.depth() == 0 {
            continue;
        }
        if listing.entries.len() >= MAX_ENTRIES {
            listing.truncated = true;
            break;
        }
        let dir = entry.file_type().is_some_and(|t| t.is_dir()) || entry.path().is_dir();
        listing.entries.push(Entry { name: entry.file_name().to_string_lossy().into_owned(), dir });
    }
    sort(&mut listing.entries);
    Ok(listing)
}

pub(super) fn sort(entries: &mut [Entry]) {
    entries.sort_by(|a, b| b.dir.cmp(&a.dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Row {
    pub path: PathBuf,
    pub name: String,
    pub dir: bool,
    pub depth: usize,
    pub open: bool,
}

struct Flatten<'a> {
    listings: &'a HashMap<PathBuf, Listing>,
    expanded: &'a HashSet<PathBuf>,
    show_hidden: bool,
}

impl Flatten<'_> {
    fn push(&self, rows: &mut Vec<Row>, folder: &Path, depth: usize) {
        let Some(listing) = self.listings.get(folder) else { return };
        for entry in &listing.entries {
            if !self.show_hidden && entry.name.starts_with('.') {
                continue;
            }
            let path = folder.join(&entry.name);
            let open = entry.dir && self.expanded.contains(&path);
            rows.push(Row { path: path.clone(), name: entry.name.clone(), dir: entry.dir, depth, open });
            if open {
                self.push(rows, &path, depth + 1);
            }
        }
    }
}

pub(super) fn rows(listings: &HashMap<PathBuf, Listing>, expanded: &HashSet<PathBuf>, show_hidden: bool) -> Vec<Row> {
    let mut rows = Vec::new();
    Flatten { listings, expanded, show_hidden }.push(&mut rows, Path::new(""), 0);
    rows
}

pub(super) fn shown_folders(expanded: &HashSet<PathBuf>) -> Vec<PathBuf> {
    let mut folders = vec![PathBuf::new()];
    folders.extend(
        expanded
            .iter()
            .filter(|p| p.ancestors().skip(1).all(|a| a.as_os_str().is_empty() || expanded.contains(a)))
            .cloned(),
    );
    folders.sort();
    folders
}

pub(super) fn reveal(expanded: &mut HashSet<PathBuf>, rel: &Path) -> Vec<PathBuf> {
    let mut opened = Vec::new();
    for parent in rel.ancestors().skip(1) {
        if parent.as_os_str().is_empty() {
            break;
        }
        if expanded.insert(parent.to_path_buf()) {
            opened.push(parent.to_path_buf());
        }
    }
    opened
}

#[cfg(test)]
mod tests;
