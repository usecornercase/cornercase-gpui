//! The tree: each open folder listed on its own, one level at a time, with the `ignore` rules
//! (`.gitignore`, `.ignore`, the global and `.git/info/exclude` ones, also from parent folders),
//! then flattened into the rows the panel draws. Listing per folder keeps a huge folder (a home
//! folder opened as a project) free until it is opened.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// The most entries one folder lists.
pub const MAX_ENTRIES: usize = 10_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub dir: bool,
}

/// One folder's entries: folders first, then files, by name ignoring case.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Listing {
    pub entries: Vec<Entry>,
    /// More than `MAX_ENTRIES` were there.
    pub truncated: bool,
}

/// Lists `root/rel` one level deep. `.git` is never listed; hidden files are (the panel hides
/// them when asked).
pub fn list(root: &Path, rel: &Path) -> std::io::Result<Listing> {
    let dir = root.join(rel);
    // Fail like `read_dir` would for a folder that is gone, rather than listing nothing.
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
        // A link to a folder opens like a folder.
        let dir = entry.file_type().is_some_and(|t| t.is_dir()) || entry.path().is_dir();
        listing.entries.push(Entry { name: entry.file_name().to_string_lossy().into_owned(), dir });
    }
    sort(&mut listing.entries);
    Ok(listing)
}

pub fn sort(entries: &mut [Entry]) {
    entries.sort_by(|a, b| b.dir.cmp(&a.dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
}

/// A row of the flattened tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// Relative to the workspace's folder.
    pub path: PathBuf,
    pub name: String,
    pub dir: bool,
    pub depth: usize,
    /// A folder that is open.
    pub open: bool,
}

/// The rows to draw: the root's entries, and under each open folder its own, when listed.
pub fn rows(listings: &HashMap<PathBuf, Listing>, expanded: &HashSet<PathBuf>, show_hidden: bool) -> Vec<Row> {
    let mut rows = Vec::new();
    push_rows(&mut rows, listings, expanded, show_hidden, Path::new(""), 0);
    rows
}

fn push_rows(
    rows: &mut Vec<Row>,
    listings: &HashMap<PathBuf, Listing>,
    expanded: &HashSet<PathBuf>,
    show_hidden: bool,
    folder: &Path,
    depth: usize,
) {
    let Some(listing) = listings.get(folder) else { return };
    for entry in &listing.entries {
        if !show_hidden && entry.name.starts_with('.') {
            continue;
        }
        let path = folder.join(&entry.name);
        let open = entry.dir && expanded.contains(&path);
        rows.push(Row { path: path.clone(), name: entry.name.clone(), dir: entry.dir, depth, open });
        if open {
            push_rows(rows, listings, expanded, show_hidden, &path, depth + 1);
        }
    }
}

/// The folders to list again: the root and every open folder whose parents are open too.
pub fn shown_folders(expanded: &HashSet<PathBuf>) -> Vec<PathBuf> {
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

/// Opens every folder above `rel`, so a file opened from a search shows in the tree.
pub fn reveal(expanded: &mut HashSet<PathBuf>, rel: &Path) -> Vec<PathBuf> {
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
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cc-gpui-tree-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn names(listing: &Listing) -> Vec<&str> {
        listing.entries.iter().map(|e| e.name.as_str()).collect()
    }

    #[test]
    fn lists_folders_first_and_respects_gitignore() {
        let root = temp("list");
        std::fs::create_dir_all(root.join(".git/info")).unwrap();
        std::fs::create_dir_all(root.join("src/deep")).unwrap();
        std::fs::create_dir_all(root.join("target")).unwrap();
        std::fs::write(root.join(".gitignore"), "target/\n*.log\n").unwrap();
        std::fs::write(root.join("b.txt"), "").unwrap();
        std::fs::write(root.join("A.md"), "").unwrap();
        std::fs::write(root.join("x.log"), "").unwrap();
        std::fs::write(root.join(".hidden"), "").unwrap();
        std::fs::write(root.join("src/main.rs"), "").unwrap();
        std::fs::write(root.join("src/debug.log"), "").unwrap();

        let top = list(&root, Path::new("")).unwrap();
        assert_eq!(names(&top), vec!["src", ".gitignore", ".hidden", "A.md", "b.txt"]);
        assert!(top.entries[0].dir);
        // The root's rules apply in a subfolder, and only one level is listed.
        let src = list(&root, Path::new("src")).unwrap();
        assert_eq!(names(&src), vec!["deep", "main.rs"]);
        assert!(list(&root, Path::new("gone")).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn flattens_open_folders_and_hides_dotfiles() {
        let mut listings = HashMap::new();
        let entry = |name: &str, dir| Entry { name: name.into(), dir };
        listings.insert(PathBuf::new(), Listing { entries: vec![entry("src", true), entry(".env", false), entry("a", false)], truncated: false });
        listings.insert(PathBuf::from("src"), Listing { entries: vec![entry("lib", true), entry("main.rs", false)], truncated: false });
        let mut expanded = HashSet::new();
        let paths = |rows: Vec<Row>| rows.into_iter().map(|r| (r.path.to_string_lossy().into_owned(), r.depth)).collect::<Vec<_>>();
        assert_eq!(paths(rows(&listings, &expanded, true)), vec![("src".into(), 0), (".env".into(), 0), ("a".into(), 0)]);
        expanded.insert(PathBuf::from("src"));
        // `src/lib` is open but not listed yet: it shows open, with nothing under it.
        expanded.insert(PathBuf::from("src/lib"));
        assert_eq!(
            paths(rows(&listings, &expanded, false)),
            vec![("src".into(), 0), ("src/lib".into(), 1), ("src/main.rs".into(), 1), ("a".into(), 0)]
        );
    }

    #[test]
    fn shown_folders_skip_closed_parents() {
        let expanded: HashSet<PathBuf> = ["a", "a/b", "c/d"].iter().map(PathBuf::from).collect();
        assert_eq!(shown_folders(&expanded), vec![PathBuf::new(), PathBuf::from("a"), PathBuf::from("a/b")]);
    }

    #[test]
    fn reveal_opens_parents() {
        let mut expanded = HashSet::new();
        expanded.insert(PathBuf::from("a"));
        assert_eq!(reveal(&mut expanded, Path::new("a/b/c.rs")), vec![PathBuf::from("a/b")]);
        assert!(expanded.contains(Path::new("a/b")));
        assert!(reveal(&mut expanded, Path::new("top.rs")).is_empty());
    }
}
