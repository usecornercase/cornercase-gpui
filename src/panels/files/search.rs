use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use grep_matcher::Matcher as _;
use grep_regex::{RegexMatcher, RegexMatcherBuilder};
use grep_searcher::{BinaryDetection, Searcher, SearcherBuilder, sinks::Lossy};
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

pub(super) const MAX_INDEX: usize = 200_000;
pub(super) const MAX_NAMES: usize = 200;
pub(super) const MAX_MATCHES: usize = 1_000;
const MAX_LINE: usize = 400;

pub(super) fn index(root: &Path, cancel: &AtomicBool) -> Option<Vec<String>> {
    let walk = ignore::WalkBuilder::new(root)
        .hidden(false)
        .require_git(false)
        .follow_links(false)
        .filter_entry(|e| e.file_name() != ".git")
        .build();
    let mut paths = Vec::new();
    for entry in walk.flatten() {
        if cancel.load(Ordering::Relaxed) {
            return None;
        }
        if paths.len() >= MAX_INDEX {
            break;
        }
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        if let Ok(rel) = entry.path().strip_prefix(root) {
            paths.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    paths.sort();
    Some(paths)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct NameHit {
    pub path: String,
    pub lit: Vec<Range<usize>>,
}

struct Ranked<'a> {
    score: u32,
    named: bool,
    name: usize,
    path: &'a String,
}

impl Ranked<'_> {
    fn order(&self, other: &Self) -> std::cmp::Ordering {
        other
            .score
            .cmp(&self.score)
            .then(other.named.cmp(&self.named))
            .then(self.name.cmp(&other.name))
            .then(self.path.len().cmp(&other.path.len()))
            .then(self.path.cmp(other.path))
    }
}

pub(super) fn names(paths: &[String], query: &str) -> Vec<NameHit> {
    let query = query.trim();
    if query.is_empty() {
        return Vec::new();
    }
    let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
    let mut buf = Vec::new();
    let mut scored: Vec<Ranked> = paths
        .iter()
        .filter_map(|path| {
            let score = pattern.score(Utf32Str::new(path, &mut buf), &mut matcher)?;
            let name = path.rsplit('/').next().unwrap_or(path);
            let named = pattern.score(Utf32Str::new(name, &mut buf), &mut matcher).is_some();
            Some(Ranked { score, named, name: name.len(), path })
        })
        .collect();
    scored.sort_by(Ranked::order);
    scored.truncate(MAX_NAMES);
    scored
        .into_iter()
        .map(|Ranked { path, .. }| {
            let mut indices = Vec::new();
            pattern.indices(Utf32Str::new(path, &mut buf), &mut matcher, &mut indices);
            indices.sort_unstable();
            indices.dedup();
            NameHit { path: path.clone(), lit: char_ranges(path, &indices) }
        })
        .collect()
}

pub(super) fn char_ranges(text: &str, indices: &[u32]) -> Vec<Range<usize>> {
    let mut ranges: Vec<Range<usize>> = Vec::new();
    let mut wanted = indices.iter().peekable();
    for (i, (at, ch)) in text.char_indices().enumerate() {
        if wanted.peek().is_some_and(|&&w| w as usize == i) {
            wanted.next();
            let range = at..at + ch.len_utf8();
            match ranges.last_mut() {
                Some(last) if last.end == range.start => last.end = range.end,
                _ => ranges.push(range),
            }
        }
    }
    ranges
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LineHit {
    pub line: u64,
    pub text: String,
    pub lit: Vec<Range<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FileHits {
    pub path: String,
    pub lines: Vec<LineHit>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct TextResults {
    pub files: Vec<FileHits>,
    pub capped: bool,
}

impl TextResults {
    pub(super) fn count(&self) -> usize {
        self.files.iter().map(|f| f.lines.len()).sum()
    }
}

pub(super) fn matcher(query: &str) -> Option<RegexMatcher> {
    let query = query.trim();
    if query.is_empty() {
        return None;
    }
    RegexMatcherBuilder::new().case_smart(true).fixed_strings(true).build(query).ok()
}

struct TextSearch<'a> {
    root: &'a Path,
    matcher: RegexMatcher,
    slots: AtomicUsize,
    capped: AtomicBool,
    cancel: &'a AtomicBool,
}

impl TextSearch<'_> {
    fn stopped(&self) -> bool {
        self.cancel.load(Ordering::Relaxed) || self.capped.load(Ordering::Relaxed)
    }

    fn take_slot(&self) -> bool {
        if self.slots.fetch_add(1, Ordering::Relaxed) >= MAX_MATCHES {
            self.capped.store(true, Ordering::Relaxed);
            return false;
        }
        true
    }

    fn file(&self, searcher: &mut Searcher, path: &str) -> Option<FileHits> {
        let mut lines = Vec::new();
        let full: PathBuf = self.root.join(path);
        let _ = searcher.search_path(
            &self.matcher,
            &full,
            Lossy(|line, text| {
                if !self.take_slot() {
                    return Ok(false);
                }
                lines.push(line_hit(&self.matcher, line, text));
                Ok(!self.cancel.load(Ordering::Relaxed))
            }),
        );
        (!lines.is_empty()).then(|| FileHits { path: path.to_string(), lines })
    }

    fn chunk(&self, chunk: &[String]) -> Vec<FileHits> {
        let mut searcher =
            SearcherBuilder::new().binary_detection(BinaryDetection::quit(b'\x00')).line_number(true).build();
        let mut found = Vec::new();
        for path in chunk {
            if self.stopped() {
                break;
            }
            found.extend(self.file(&mut searcher, path));
        }
        found
    }
}

pub(super) fn text(root: &Path, paths: &[String], query: &str, cancel: &AtomicBool) -> TextResults {
    let Some(matcher) = matcher(query) else { return TextResults::default() };
    let search = TextSearch { root, matcher, slots: AtomicUsize::new(0), capped: AtomicBool::new(false), cancel };
    let threads = std::thread::available_parallelism().map_or(4, std::num::NonZero::get).min(8);
    let chunk = paths.len().div_ceil(threads).max(1);
    let mut files: Vec<FileHits> = std::thread::scope(|scope| {
        let workers: Vec<_> = paths.chunks(chunk).map(|chunk| scope.spawn(|| search.chunk(chunk))).collect();
        workers.into_iter().flat_map(|w| w.join().unwrap_or_default()).collect()
    });
    files.sort_by(|a, b| a.path.cmp(&b.path));
    TextResults { files, capped: search.capped.load(Ordering::Relaxed) }
}

fn line_hit(matcher: &RegexMatcher, line: u64, text: &str) -> LineHit {
    let text = text.trim_end_matches(['\n', '\r']).replace('\t', "    ");
    let text = text.trim_start();
    let mut lit = Vec::new();
    let _ = matcher.find_iter(text.as_bytes(), |m| {
        lit.push(m.start()..m.end());
        true
    });
    let mut start = lit.first().map_or(0, |r| r.start.saturating_sub(40));
    while !text.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = text.len();
    if let Some((i, _)) = text[start..].char_indices().nth(MAX_LINE) {
        end = start + i;
    }
    let mut cut = String::new();
    let shift = if start > 0 {
        cut.push('…');
        '…'.len_utf8()
    } else {
        0
    };
    cut.push_str(&text[start..end]);
    if end < text.len() {
        cut.push('…');
    }
    let lit = lit
        .into_iter()
        .filter(|r| r.start >= start && r.end <= end)
        .map(|r| r.start - start + shift..r.end - start + shift)
        .collect();
    LineHit { line, text: cut, lit }
}

#[cfg(test)]
mod tests;
