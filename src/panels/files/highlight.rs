use std::ops::Range;
use std::path::Path;
use std::sync::OnceLock;

use syntect::parsing::{ParseState, Scope, ScopeStack, ScopeStackOp, SyntaxReference, SyntaxSet};

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

pub(super) type Span = (Range<usize>, Kind);

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

fn kind(stack: &ScopeStack) -> Option<Kind> {
    stack
        .as_slice()
        .iter()
        .rev()
        .find_map(|scope| rules().iter().find(|(prefix, _)| prefix.is_prefix_of(*scope)).map(|(_, k)| *k))
}

fn syntax_for(path: &Path, first_line: &str) -> Option<&'static SyntaxReference> {
    let set = syntaxes();
    let name = path.file_name()?.to_str()?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let ext = match ext {
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

pub(super) fn language(path: &Path, first_line: &str) -> Option<&'static str> {
    syntax_for(path, first_line).map(|s| s.name.as_str())
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
            out.resize(lines.len(), Vec::new());
            return Some(out);
        };
        let spans = line_spans(ops, &mut stack, line.len());
        out.push(spans);
    }
    Some(out)
}

#[cfg(test)]
mod tests;
