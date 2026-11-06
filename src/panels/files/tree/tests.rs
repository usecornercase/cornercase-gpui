use super::*;

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cc-gpui-tree-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the temporary files are writable");
    dir
}

fn names(listing: &Listing) -> Vec<&str> {
    listing.entries.iter().map(|e| e.name.as_str()).collect()
}

#[test]
fn lists_folders_first_and_respects_gitignore() {
    let root = temp("list");
    std::fs::create_dir_all(root.join(".git/info")).expect("the temporary files are writable");
    std::fs::create_dir_all(root.join("src/deep")).expect("the temporary files are writable");
    std::fs::create_dir_all(root.join("target")).expect("the temporary files are writable");
    std::fs::write(root.join(".gitignore"), "target/\n*.log\n").expect("the temporary files are writable");
    std::fs::write(root.join("b.txt"), "").expect("the temporary files are writable");
    std::fs::write(root.join("A.md"), "").expect("the temporary files are writable");
    std::fs::write(root.join("x.log"), "").expect("the temporary files are writable");
    std::fs::write(root.join(".hidden"), "").expect("the temporary files are writable");
    std::fs::write(root.join("src/main.rs"), "").expect("the temporary files are writable");
    std::fs::write(root.join("src/debug.log"), "").expect("the temporary files are writable");

    let top = list(&root, Path::new("")).expect("the folder lists");
    assert_eq!(names(&top), vec!["src", ".gitignore", ".hidden", "A.md", "b.txt"]);
    assert!(top.entries[0].dir);
    let src = list(&root, Path::new("src")).expect("the folder lists");
    assert_eq!(names(&src), vec!["deep", "main.rs"]);
    assert!(list(&root, Path::new("gone")).is_err());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn flattens_open_folders_and_hides_dotfiles() {
    let mut listings = HashMap::new();
    let entry = |name: &str, dir| Entry { name: name.into(), dir, status: None };
    listings.insert(
        PathBuf::new(),
        Listing { entries: vec![entry("src", true), entry(".env", false), entry("a", false)], truncated: false },
    );
    listings.insert(
        PathBuf::from("src"),
        Listing { entries: vec![entry("lib", true), entry("main.rs", false)], truncated: false },
    );
    let mut expanded = HashSet::new();
    let paths = |rows: Vec<Row>| rows.into_iter().map(|r| (r.path, r.depth)).collect::<Vec<_>>();
    let at = |path: &str, depth: usize| (PathBuf::from(path), depth);
    assert_eq!(paths(rows(&listings, &expanded, true)), vec![at("src", 0), at(".env", 0), at("a", 0)]);
    expanded.insert(PathBuf::from("src"));
    expanded.insert(PathBuf::from("src/lib"));
    let src = Path::new("src");
    assert_eq!(
        paths(rows(&listings, &expanded, false)),
        vec![at("src", 0), (src.join("lib"), 1), (src.join("main.rs"), 1), at("a", 0)]
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
    assert_eq!(reveal(&mut expanded, Path::new("top.rs")), Vec::<PathBuf>::new());
}

fn marked(entries: &[(&str, Option<FileStatus>)]) -> Listing {
    let entry =
        |(name, status): &(&str, Option<FileStatus>)| Entry { name: (*name).into(), dir: false, status: *status };
    Listing { entries: entries.iter().map(entry).collect(), truncated: false }
}

#[test]
fn the_marks_of_a_listing_are_kept_by_path() {
    let mut marks = Marks::new();
    let listing = marked(&[("a.rs", Some(FileStatus::Modified)), ("b.rs", None)]);
    assert!(keep_marks(&mut marks, Path::new("src"), &listing));
    assert_eq!(marks.get(&Path::new("src").join("a.rs")), Some(&FileStatus::Modified));
    assert_eq!(marks.len(), 1);
}

#[test]
fn a_listing_with_the_same_marks_changes_nothing() {
    let mut marks = Marks::new();
    let listing = marked(&[("a.rs", Some(FileStatus::Modified))]);
    keep_marks(&mut marks, Path::new(""), &listing);
    assert!(!keep_marks(&mut marks, Path::new(""), &listing));
}

#[test]
fn a_file_that_is_no_longer_changed_loses_its_mark() {
    let mut marks = Marks::new();
    keep_marks(&mut marks, Path::new(""), &marked(&[("a.rs", Some(FileStatus::Modified))]));
    assert!(keep_marks(&mut marks, Path::new(""), &marked(&[("a.rs", None)])));
    assert!(marks.is_empty());
}

#[test]
fn a_mark_that_changes_kind_is_replaced() {
    let mut marks = Marks::new();
    keep_marks(&mut marks, Path::new(""), &marked(&[("a.rs", Some(FileStatus::Untracked))]));
    assert!(keep_marks(&mut marks, Path::new(""), &marked(&[("a.rs", Some(FileStatus::Added))])));
    assert_eq!(marks.get(Path::new("a.rs")), Some(&FileStatus::Added));
}

#[test]
fn a_listing_leaves_the_marks_of_other_folders() {
    let mut marks = Marks::new();
    keep_marks(&mut marks, Path::new("src"), &marked(&[("a.rs", Some(FileStatus::Modified))]));
    keep_marks(&mut marks, Path::new(""), &marked(&[("src", Some(FileStatus::Modified))]));
    assert!(!keep_marks(&mut marks, Path::new("docs"), &marked(&[])));
    assert_eq!(marks.len(), 2);
}
