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

fn fence(lang: &str, text: &str) -> Vec<(String, Kind)> {
    fenced(lang, text).iter().map(|(range, kind)| (text[range.clone()].to_string(), *kind)).collect()
}

fn has(spans: &[(String, Kind)], text: &str, kind: Kind) -> bool {
    spans.contains(&(text.to_string(), kind))
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

#[test]
fn a_fenced_rust_block_gets_its_kinds() {
    let spans = fence("rust", "// hi\nfn main() { let x = \"s\"; 42 }");
    assert!(has(&spans, "// hi", Kind::Comment));
    assert!(has(&spans, "fn", Kind::Keyword));
    assert!(has(&spans, "main", Kind::Function));
    assert!(has(&spans, "\"s\"", Kind::String));
    assert!(has(&spans, "42", Kind::Number));
}

#[test]
fn a_command_is_coloured_by_its_name_and_not_its_arguments() {
    let spans = fence("sh", "# build\ncargo test --locked && echo \"done\"");
    assert!(has(&spans, "# build", Kind::Comment));
    assert!(has(&spans, "cargo", Kind::Function));
    assert!(has(&spans, "echo", Kind::Function));
    assert!(has(&spans, "&&", Kind::Keyword));
    assert!(has(&spans, "\"done\"", Kind::String));
    assert!(!spans.iter().any(|(text, _)| text.contains("test")));
}

#[test]
fn json_strings_numbers_and_constants() {
    let spans = fence("json", "{\"name\": 3, \"ok\": true}");
    assert!(has(&spans, "\"name\"", Kind::String));
    assert!(has(&spans, "3", Kind::Number));
    assert!(has(&spans, "true", Kind::Number));
}

#[test]
fn typescript_and_javascript_share_a_grammar() {
    let text = "const x = 3;";
    let js = fenced("js", text);
    for lang in ["ts", "typescript", "tsx", "javascript", "JS"] {
        assert_eq!(fenced(lang, text), js, "{lang}");
    }
    let spans = fence("ts", text);
    assert!(has(&spans, "const", Kind::Keyword));
    assert!(has(&spans, "3", Kind::Number));
}

#[test]
fn python_gets_its_kinds() {
    let spans = fence("python", "# c\ndef f():\n    return \"s\"");
    assert!(has(&spans, "# c", Kind::Comment));
    assert!(has(&spans, "def", Kind::Keyword));
    assert!(has(&spans, "f", Kind::Function));
    assert!(has(&spans, "\"s\"", Kind::String));
}

#[test]
fn a_diff_tells_added_from_removed() {
    let spans = fence("diff", "--- a/x\n+++ b/x\n@@ -1,2 +1,2 @@\n-old\n+new\n same");
    assert!(has(&spans, "-old", Kind::Removed));
    assert!(has(&spans, "+new", Kind::Added));
    assert!(has(&spans, "@@ -1,2 +1,2 @@", Kind::Function));
    assert!(has(&spans, "--- a/x", Kind::Keyword));
}

#[test]
fn yaml_keys_and_values() {
    let spans = fence("yaml", "# c\nname: x\ncount: 3");
    assert!(has(&spans, "# c", Kind::Comment));
    assert!(has(&spans, "name", Kind::Tag));
    assert!(has(&spans, "x", Kind::String));
    assert!(has(&spans, "3", Kind::Number));
}

#[test]
fn toml_through_a_fence() {
    let spans = fence("toml", "# c\n[package]\nname = \"x\"\nn = 3\nok = true");
    assert!(has(&spans, "# c", Kind::Comment));
    assert!(has(&spans, "[package]", Kind::Function));
    assert!(has(&spans, "name", Kind::Tag));
    assert!(has(&spans, "\"x\"", Kind::String));
    assert!(has(&spans, "3", Kind::Number));
    assert!(has(&spans, "true", Kind::Number));
}

#[test]
fn no_language_or_an_unknown_one_gives_no_spans() {
    for lang in ["", "nope", "text", "txt"] {
        assert!(fenced(lang, "fn main() {}").is_empty(), "{lang}");
    }
}

#[test]
fn a_block_over_the_limit_stays_plain() {
    let text = "let a = 1;\n".repeat(MAX_BLOCK_BYTES / 10);
    assert!(text.len() > MAX_BLOCK_BYTES);
    assert_eq!(fenced("rust", &text), Vec::<Span>::new());
    assert_ne!(fenced("rust", &text[..MAX_BLOCK_BYTES - 10]), Vec::<Span>::new());
}

#[test]
fn spans_are_sorted_apart_and_on_char_boundaries() {
    let text = "# é ✓ // é\n\"é ✓\" 'é' x = \"✓\" # ✓\n+é\n-✓\nkey: é ✓";
    for lang in ["rust", "sh", "json", "toml", "ts", "python", "diff", "yaml"] {
        let spans = fenced(lang, text);
        assert!(!spans.is_empty(), "{lang}");
        for (range, _) in &spans {
            assert!(range.start < range.end && range.end <= text.len(), "{lang}: {range:?}");
            assert!(text.is_char_boundary(range.start) && text.is_char_boundary(range.end), "{lang}: {range:?}");
        }
        assert!(spans.windows(2).all(|pair| pair[0].0.end <= pair[1].0.start), "{lang}: {spans:?}");
    }
}

#[test]
fn a_toml_file_is_coloured_in_the_viewer() {
    assert_eq!(language(Path::new("Cargo.toml"), ""), Some("TOML"));
    assert_eq!(language(Path::new("Cargo.lock"), ""), Some("TOML"));
    let lines = kinds("Cargo.toml", "[package]\nname = \"x\"");
    assert_eq!(lines[0], vec![("[package]".to_string(), Kind::Function)]);
    assert_eq!(lines[1], vec![("name".to_string(), Kind::Tag), ("\"x\"".to_string(), Kind::String)]);
}
