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
    let entry = |name: &str, dir| Entry { name: name.into(), dir };
    listings.insert(
        PathBuf::new(),
        Listing { entries: vec![entry("src", true), entry(".env", false), entry("a", false)], truncated: false },
    );
    listings.insert(
        PathBuf::from("src"),
        Listing { entries: vec![entry("lib", true), entry("main.rs", false)], truncated: false },
    );
    let mut expanded = HashSet::new();
    let paths =
        |rows: Vec<Row>| rows.into_iter().map(|r| (r.path.to_string_lossy().into_owned(), r.depth)).collect::<Vec<_>>();
    assert_eq!(paths(rows(&listings, &expanded, true)), vec![("src".into(), 0), (".env".into(), 0), ("a".into(), 0)]);
    expanded.insert(PathBuf::from("src"));
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
    assert_eq!(reveal(&mut expanded, Path::new("top.rs")), Vec::<PathBuf>::new());
}
