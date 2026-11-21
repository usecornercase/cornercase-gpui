mod toml;

use std::ops::Range;
use std::path::Path;
use std::sync::OnceLock;

use syntect::parsing::{ParseState, Scope, ScopeStack, ScopeStackOp, SyntaxReference, SyntaxSet};

pub(crate) const MAX_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_BLOCK_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Keyword,
    Function,
    Type,
    String,
    Number,
    Comment,
    Tag,
    Added,
    Removed,
}

impl Kind {
    pub(crate) fn ansi(self) -> Option<u8> {
        match self {
            Kind::Keyword => Some(5),
            Kind::Function => Some(4),
            Kind::Type => Some(6),
            Kind::String | Kind::Added => Some(2),
            Kind::Number => Some(3),
            Kind::Tag | Kind::Removed => Some(1),
            Kind::Comment => None,
        }
    }
}

pub(crate) type Span = (Range<usize>, Kind);

enum Grammar {
    Syntect(&'static SyntaxReference),
    Toml,
}

fn syntaxes() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

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
            ("markup.inserted", Kind::Added),
            ("markup.deleted", Kind::Removed),
            ("meta.diff.range", Kind::Function),
            ("meta.diff.header", Kind::Keyword),
            ("keyword", Kind::Keyword),
            ("storage", Kind::Keyword),
        ];
        table.iter().filter_map(|(s, k)| Scope::new(s).ok().map(|s| (s, *k))).collect()
    })
}

fn kind(stack: &ScopeStack) -> Option<Kind> {
    stack
        .as_slice()
        .iter()
        .rev()
        .find_map(|scope| rules().iter().find(|(prefix, _)| prefix.is_prefix_of(*scope)).map(|(_, k)| *k))
}

fn relative(word: &str) -> &str {
    match word {
        "ts" | "tsx" | "mts" | "cts" | "jsx" | "mjs" | "cjs" | "typescript" | "javascript" | "node" => "js",
        "bash" | "zsh" | "fish" | "shell" | "console" | "shellsession" | "env" => "sh",
        "jsonc" | "json5" => "json",
        "kotlin" | "kt" | "kts" | "swift" => "java",
        "vue" | "svelte" => "html",
        "golang" => "go",
        "csharp" => "cs",
        "python3" => "py",
        other => other,
    }
}

fn not_plain(syntax: &'static SyntaxReference) -> Option<Grammar> {
    (syntax.name != "Plain Text").then_some(Grammar::Syntect(syntax))
}

fn grammar_for(path: &Path, first_line: &str) -> Option<Grammar> {
    let set = syntaxes();
    let name = path.file_name()?.to_str()?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if ext == "toml" || name == "Cargo.lock" {
        return Some(Grammar::Toml);
    }
    set.find_syntax_by_extension(name)
        .or_else(|| set.find_syntax_by_extension(relative(ext)))
        .or_else(|| set.find_syntax_by_first_line(first_line))
        .and_then(not_plain)
}

fn grammar_named(lang: &str) -> Option<Grammar> {
    let lang = lang.to_lowercase();
    match lang.as_str() {
        "" => None,
        "toml" => Some(Grammar::Toml),
        other => syntaxes().find_syntax_by_token(relative(other)).and_then(not_plain),
    }
}

pub(crate) fn language(path: &Path, first_line: &str) -> Option<&'static str> {
    grammar_for(path, first_line).map(|grammar| match grammar {
        Grammar::Syntect(syntax) => syntax.name.as_str(),
        Grammar::Toml => "TOML",
    })
}

fn push_span(spans: &mut Vec<Span>, range: Range<usize>, stack: &ScopeStack, len: usize) {
    let range = range.start.min(len)..range.end.min(len);
    if range.is_empty() {
        return;
    }
    let Some(kind) = kind(stack) else { return };
    match spans.last_mut() {
        Some((last, k)) if *k == kind && last.end == range.start => last.end = range.end,
        _ => spans.push((range, kind)),
    }
}

fn line_spans(ops: Vec<(usize, ScopeStackOp)>, stack: &mut ScopeStack, len: usize) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut start = 0;
    for (at, op) in ops {
        push_span(&mut spans, start..at, stack, len);
        start = at;
        if stack.apply(&op).is_err() {
            break;
        }
    }
    push_span(&mut spans, start..len + 1, stack, len);
    spans
}

fn parsed(syntax: &SyntaxReference, lines: &[&str]) -> Vec<Vec<Span>> {
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
            out.resize(lines.len(), Vec::new());
            return out;
        };
        out.push(line_spans(ops, &mut stack, line.len()));
    }
    out
}

fn painted(grammar: &Grammar, lines: &[&str]) -> Vec<Vec<Span>> {
    match grammar {
        Grammar::Syntect(syntax) => parsed(syntax, lines),
        Grammar::Toml => toml::spans(lines),
    }
}

pub(crate) fn highlight(path: &Path, lines: &[String]) -> Option<Vec<Vec<Span>>> {
    let bytes: usize = lines.iter().map(|l| l.len() + 1).sum();
    if bytes > MAX_BYTES {
        return None;
    }
    let grammar = grammar_for(path, lines.first().map_or("", String::as_str))?;
    let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
    Some(painted(&grammar, &lines))
}

pub(crate) fn fenced(lang: &str, text: &str) -> Vec<Span> {
    if text.len() > MAX_BLOCK_BYTES {
        return Vec::new();
    }
    let Some(grammar) = grammar_named(lang) else { return Vec::new() };
    let lines: Vec<&str> = text.split('\n').collect();
    let mut start = 0;
    let mut out = Vec::new();
    for (line, spans) in lines.iter().zip(painted(&grammar, &lines)) {
        out.extend(spans.into_iter().map(|(range, kind)| (range.start + start..range.end + start, kind)));
        start += line.len() + 1;
    }
    out
}

#[cfg(test)]
mod tests;
