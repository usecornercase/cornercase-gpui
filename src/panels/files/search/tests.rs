use super::*;

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cc-gpui-search-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the temporary files are writable");
    dir
}

#[test]
fn index_skips_ignored_and_git() {
    let root = temp("index");
    std::fs::create_dir_all(root.join(".git")).expect("the temporary files are writable");
    std::fs::create_dir_all(root.join("src")).expect("the temporary files are writable");
    std::fs::create_dir_all(root.join("target")).expect("the temporary files are writable");
    std::fs::write(root.join(".gitignore"), "target/\n").expect("the temporary files are writable");
    std::fs::write(root.join(".git/HEAD"), "x").expect("the temporary files are writable");
    std::fs::write(root.join("target/out"), "x").expect("the temporary files are writable");
    std::fs::write(root.join("src/a.rs"), "x").expect("the temporary files are writable");
    std::fs::write(root.join("README"), "x").expect("the temporary files are writable");
    let paths = index(&root, &AtomicBool::new(false));
    assert_eq!(paths.expect("the walk ends"), vec![".gitignore", "README", "src/a.rs"]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_cancelled_walk_gives_no_index() {
    let root = temp("cancelled");
    std::fs::write(root.join("a.rs"), "x").expect("the temporary files are writable");
    assert_eq!(index(&root, &AtomicBool::new(true)), None);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn names_prefer_the_named_file() {
    let paths: Vec<String> =
        ["src/relay/workspace-session-handler.ts", "src/renderer/src/lib/workspace-session.ts", "docs/other.md"]
            .iter()
            .map(ToString::to_string)
            .collect();
    let hits = names(&paths, "workspace-session");
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].path, "src/renderer/src/lib/workspace-session.ts");
    let lit: String = hits[0].lit.iter().map(|r| &hits[0].path[r.clone()]).collect();
    assert_eq!(lit, "workspace-session");
    assert_eq!(names(&paths, "  "), Vec::new());
}

#[test]
fn names_prefer_a_file_whose_own_name_matches_to_files_in_a_folder_of_that_name() {
    let paths: Vec<String> =
        ["src/panels/files/finder/view.rs", "src/panels/files/finder/tests.rs", "src/panels/files/finder.rs"]
            .iter()
            .map(ToString::to_string)
            .collect();
    let hits: Vec<String> = names(&paths, "finder").into_iter().map(|h| h.path).collect();
    assert_eq!(
        hits,
        ["src/panels/files/finder.rs", "src/panels/files/finder/view.rs", "src/panels/files/finder/tests.rs"]
    );
}

#[test]
fn char_ranges_merge_and_respect_utf8() {
    assert_eq!(char_ranges("aéb", &[1, 2]), vec![1..4]);
    assert_eq!(char_ranges("abc", &[0, 2]), vec![0..1, 2..3]);
}

#[test]
fn text_search_is_literal_smart_case_and_skips_binary() {
    let root = temp("text");
    std::fs::write(root.join("a.txt"), "one Foo\n\tfoo(bar)\nnone\n").expect("the temporary files are writable");
    std::fs::write(root.join("b.bin"), b"foo\0foo").expect("the temporary files are writable");
    std::fs::write(root.join("c.txt"), "FOO\n").expect("the temporary files are writable");
    let paths = vec!["a.txt".to_string(), "b.bin".to_string(), "c.txt".to_string()];
    let cancel = AtomicBool::new(false);

    let found = text(&root, &paths, "foo(", &cancel);
    assert_eq!(found.files.len(), 1);
    assert_eq!(
        found.files[0].lines,
        vec![LineHit { line: 2, text: "foo(bar)".into(), lit: vec![Range { start: 0, end: 4 }] }]
    );

    let found = text(&root, &paths, "foo", &cancel);
    assert_eq!(found.files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), vec!["a.txt", "c.txt"]);
    assert_eq!(found.count(), 3);
    assert_eq!(text(&root, &paths, "Foo", &cancel).count(), 1);
    assert_eq!(text(&root, &paths, " ", &cancel).files, Vec::new());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn text_search_caps_matches() {
    let root = temp("cap");
    std::fs::write(root.join("many.txt"), "x\n".repeat(MAX_MATCHES + 5)).expect("the temporary files are writable");
    let found = text(&root, &["many.txt".to_string()], "x", &AtomicBool::new(false));
    assert_eq!(found.count(), MAX_MATCHES);
    assert!(found.capped);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn long_lines_are_cut_around_the_match() {
    let matcher = matcher("needle").expect("a matcher");
    let line = format!("{}needle{}", "a".repeat(1000), "b".repeat(1000));
    let hit = line_hit(&matcher, 1, &line);
    assert!(hit.text.starts_with('…') && hit.text.ends_with('…'));
    assert_eq!(&hit.text[hit.lit[0].clone()], "needle");
}
