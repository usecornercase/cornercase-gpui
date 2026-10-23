//! Syntax colours for the viewer, with syntect's default grammars. Scopes map to a few kinds
//! drawn in the palette's colours, as the TUI does: keyword magenta, function blue, type cyan,
//! string green, number and constant yellow, property and tag red, comment grey in italics.

use std::ops::Range;
use std::path::Path;
use std::sync::OnceLock;

use syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxReference, SyntaxSet};

/// Sources larger than this stay plain: syntect's pure-Rust regexes are slow on big files.
pub(super) const MAX_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Keyword,
    Function,
    Type,
    String,
    Number,
    Comment,
    Tag,
}

/// A coloured byte range of a drawn line.
pub(super) type Span = (Range<usize>, Kind);

fn syntaxes() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// Scope prefixes, most specific first.
fn rules() -> &'static [(Scope, Kind)] {
    static RULES: OnceLock<Vec<(Scope, Kind)>> = OnceLock::new();
    RULES.get_or_init(|| {
        let table: &[(&str, Kind)] = &[
            ("comment", Kind::Comment),
            ("punctuation.definition.comment", Kind::Comment),
            ("string", Kind::String),
            ("constant.character", Kind::String),
            ("markup.raw", Kind::String),
            ("constant.numeric", Kind::Number),
            ("constant.language", Kind::Number),
            ("constant.other", Kind::Number),
            ("variable.language", Kind::Number),
            ("entity.name.function", Kind::Function),
            ("support.function", Kind::Function),
            ("variable.function", Kind::Function),
            ("meta.function-call", Kind::Function),
            ("markup.heading", Kind::Function),
            ("entity.name.type", Kind::Type),
            ("entity.name.class", Kind::Type),
            ("entity.name.struct", Kind::Type),
            ("entity.name.enum", Kind::Type),
            ("entity.name.trait", Kind::Type),
            ("entity.other.inherited-class", Kind::Type),
            ("support.type", Kind::Type),
            ("support.class", Kind::Type),
            ("storage.type.primitive", Kind::Type),
            ("storage.type.numeric", Kind::Type),
            ("entity.name.tag", Kind::Tag),
            ("entity.other.attribute-name", Kind::Tag),
            ("meta.mapping.key", Kind::Tag),
            ("variable.other.member", Kind::Tag),
            ("variable.other.property", Kind::Tag),
            ("keyword", Kind::Keyword),
            ("storage", Kind::Keyword),
        ];
        table.iter().filter_map(|(s, k)| Scope::new(s).ok().map(|s| (s, *k))).collect()
    })
}

/// The kind of the innermost scope that has one.
fn kind(stack: &ScopeStack) -> Option<Kind> {
    stack
        .as_slice()
        .iter()
        .rev()
        .find_map(|scope| rules().iter().find(|(prefix, _)| prefix.is_prefix_of(*scope)).map(|(_, k)| *k))
}

/// The grammar for a file: by its name (`Makefile`), its extension, then its first line.
fn syntax_for(path: &Path, first_line: &str) -> Option<&'static SyntaxReference> {
    let set = syntaxes();
    let name = path.file_name()?.to_str()?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let ext = match ext {
        // Close relatives for what the default set lacks.
        "ts" | "tsx" | "mts" | "cts" | "jsx" | "mjs" | "cjs" => "js",
        "toml" | "ini" | "cfg" | "conf" => "ini",
        "zsh" | "fish" | "env" => "sh",
        "jsonc" | "json5" => "json",
        "kt" | "kts" | "swift" => "java",
        "vue" | "svelte" => "html",
        other => other,
    };
    set.find_syntax_by_extension(name)
        .or_else(|| set.find_syntax_by_extension(ext))
        .or_else(|| set.find_syntax_by_first_line(first_line))
        .filter(|s| s.name != "Plain Text")
}

/// The name of the language a file is coloured as, for the header.
pub(super) fn language(path: &Path, first_line: &str) -> Option<&'static str> {
    syntax_for(path, first_line).map(|s| s.name.as_str())
}

/// Colours each drawn line, or `None` when the file has no grammar or is too large. Lines are
/// parsed one after another, so the state carries across them (block comments, strings).
pub(super) fn highlight(path: &Path, lines: &[String]) -> Option<Vec<Vec<Span>>> {
    let bytes: usize = lines.iter().map(|l| l.len() + 1).sum();
    if bytes > MAX_BYTES {
        return None;
    }
    let syntax = syntax_for(path, lines.first().map_or("", String::as_str))?;
    let set = syntaxes();
    let mut state = ParseState::new(syntax);
    let mut stack = ScopeStack::new();
    let mut out = Vec::with_capacity(lines.len());
    let mut buf = String::new();
    for line in lines {
        buf.clear();
        buf.push_str(line);
        buf.push('\n');
        let Ok(ops) = state.parse_line(&buf, set) else {
            // A grammar that fails leaves the rest plain.
            out.resize(lines.len(), Vec::new());
            return Some(out);
        };
        let mut spans: Vec<Span> = Vec::new();
        let mut start = 0;
        let push = |range: Range<usize>, stack: &ScopeStack, spans: &mut Vec<Span>| {
            let range = range.start.min(line.len())..range.end.min(line.len());
            if range.is_empty() {
                return;
            }
            if let Some(kind) = kind(stack) {
                match spans.last_mut() {
                    Some((last, k)) if *k == kind && last.end == range.start => last.end = range.end,
                    _ => spans.push((range, kind)),
                }
            }
        };
        for (at, op) in ops {
            push(start..at, &stack, &mut spans);
            start = at;
            if stack.apply(&op).is_err() {
                break;
            }
        }
        push(start..buf.len(), &stack, &mut spans);
        out.push(spans);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
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
}
