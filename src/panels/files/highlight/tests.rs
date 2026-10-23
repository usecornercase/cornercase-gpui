use super::*;

fn kinds(path: &str, src: &str) -> Vec<Vec<(String, Kind)>> {
    let lines: Vec<String> = src.lines().map(str::to_string).collect();
    let spans = highlight(Path::new(path), &lines).expect("a grammar");
    spans
        .iter()
        .zip(&lines)
        .map(|(spans, line)| spans.iter().map(|(r, k)| (line[r.clone()].to_string(), *k)).collect())
        .collect()
}

#[test]
fn colours_rust() {
    let lines = kinds("main.rs", "// hi\nfn main() { let x = \"s\"; 42 }");
    assert_eq!(lines[0], vec![("// hi".to_string(), Kind::Comment)]);
    assert!(lines[1].contains(&("fn".to_string(), Kind::Keyword)));
    assert!(lines[1].contains(&("main".to_string(), Kind::Function)));
    assert!(lines[1].contains(&("\"s\"".to_string(), Kind::String)));
    assert!(lines[1].contains(&("42".to_string(), Kind::Number)));
}

#[test]
fn finds_grammars_by_name_and_shebang() {
    assert_eq!(language(Path::new("Makefile"), ""), Some("Makefile"));
    assert_eq!(language(Path::new("run"), "#!/bin/bash"), Some("Bourne Again Shell (bash)"));
    assert_eq!(language(Path::new("app.tsx"), ""), Some("JavaScript"));
    assert_eq!(language(Path::new("notes.unknownext"), "hello"), None);
}

#[test]
fn large_sources_stay_plain() {
    let lines = vec!["x".repeat(MAX_BYTES)];
    assert!(highlight(Path::new("a.rs"), &lines).is_none());
}
