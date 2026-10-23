//! The panel's search: an index of the workspace's file paths (one `ignore` walk), fuzzy file
//! names over it (nucleo's matcher, fzf's scoring) and text in the files (ripgrep's searcher, the
//! query a literal with smart case, binary files skipped), as the TUI's files panel does.

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use grep_matcher::Matcher as _;
use grep_regex::{RegexMatcher, RegexMatcherBuilder};
use grep_searcher::{BinaryDetection, SearcherBuilder, sinks::Lossy};
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

/// The most paths the index holds.
pub const MAX_INDEX: usize = 200_000;
/// The most file name results.
pub const MAX_NAMES: usize = 200;
/// The most text matches.
pub const MAX_MATCHES: usize = 1_000;
/// A matched line is cut to this many characters.
const MAX_LINE: usize = 400;

/// Every file under `root` the tree would list (hidden included, ignored and `.git` not), as
/// relative paths with `/`, sorted.
pub fn index(root: &Path, cancel: &AtomicBool) -> Vec<String> {
    let walk = ignore::WalkBuilder::new(root)
        .hidden(false)
        .require_git(false)
        .follow_links(false)
        .filter_entry(|e| e.file_name() != ".git")
        .build();
    let mut paths = Vec::new();
    for entry in walk.flatten() {
        if paths.len() >= MAX_INDEX || cancel.load(Ordering::Relaxed) {
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
    paths
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameHit {
    pub path: String,
    /// Byte ranges of `path` that matched, for lighting.
    pub lit: Vec<Range<usize>>,
}

/// The best `MAX_NAMES` paths for a fuzzy query, smart case; on a tie the shorter file name,
/// then the shorter path, so the file a query names comes first.
pub fn names(paths: &[String], query: &str) -> Vec<NameHit> {
    let query = query.trim();
    if query.is_empty() {
        return Vec::new();
    }
    let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
    let mut buf = Vec::new();
    let mut scored: Vec<(u32, usize, &String)> = paths
        .iter()
        .filter_map(|path| {
            let score = pattern.score(Utf32Str::new(path, &mut buf), &mut matcher)?;
            let name = path.rsplit('/').next().map_or(path.len(), str::len);
            Some((score, name, path))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.len().cmp(&b.2.len())).then(a.2.cmp(b.2)));
    scored.truncate(MAX_NAMES);
    scored
        .into_iter()
        .map(|(_, _, path)| {
            let mut indices = Vec::new();
            pattern.indices(Utf32Str::new(path, &mut buf), &mut matcher, &mut indices);
            indices.sort_unstable();
            indices.dedup();
            NameHit { path: path.clone(), lit: char_ranges(path, &indices) }
        })
        .collect()
}

/// Character indices as merged byte ranges.
fn char_ranges(text: &str, indices: &[u32]) -> Vec<Range<usize>> {
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
pub struct LineHit {
    /// 1-based.
    pub line: u64,
    /// The line without its indent, cut to `MAX_LINE` characters around the first match.
    pub text: String,
    pub lit: Vec<Range<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileHits {
    pub path: String,
    pub lines: Vec<LineHit>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextResults {
    /// By path.
    pub files: Vec<FileHits>,
    /// A match was left out past `MAX_MATCHES`.
    pub capped: bool,
}

impl TextResults {
    pub fn count(&self) -> usize {
        self.files.iter().map(|f| f.lines.len()).sum()
    }
}

/// The query as a literal with smart case; `None` for a blank one.
pub fn matcher(query: &str) -> Option<RegexMatcher> {
    let query = query.trim();
    if query.is_empty() {
        return None;
    }
    RegexMatcherBuilder::new().case_smart(true).fixed_strings(true).build(query).ok()
}

/// Searches the text of the indexed files on up to 8 threads; `cancel` stops it early.
pub fn text(root: &Path, paths: &[String], query: &str, cancel: &AtomicBool) -> TextResults {
    let Some(matcher) = matcher(query) else { return TextResults::default() };
    let slots = AtomicUsize::new(0);
    let capped = AtomicBool::new(false);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(8);
    let chunk = paths.len().div_ceil(threads).max(1);
    let mut files: Vec<FileHits> = std::thread::scope(|scope| {
        let workers: Vec<_> = paths
            .chunks(chunk)
            .map(|chunk| {
                let (matcher, slots, capped) = (&matcher, &slots, &capped);
                scope.spawn(move || {
                    let mut searcher = SearcherBuilder::new()
                        .binary_detection(BinaryDetection::quit(b'\x00'))
                        .line_number(true)
                        .build();
                    let mut found = Vec::new();
                    for path in chunk {
                        if cancel.load(Ordering::Relaxed) || capped.load(Ordering::Relaxed) {
                            break;
                        }
                        let mut lines = Vec::new();
                        let full: PathBuf = root.join(path);
                        let _ = searcher.search_path(
                            matcher,
                            &full,
                            Lossy(|line, text| {
                                // Each match takes a slot before it is kept, so the threads
                                // together never keep more than the cap.
                                if slots.fetch_add(1, Ordering::Relaxed) >= MAX_MATCHES {
                                    capped.store(true, Ordering::Relaxed);
                                    return Ok(false);
                                }
                                lines.push(line_hit(matcher, line, text));
                                Ok(!cancel.load(Ordering::Relaxed))
                            }),
                        );
                        if !lines.is_empty() {
                            found.push(FileHits { path: path.clone(), lines });
                        }
                    }
                    found
                })
            })
            .collect();
        workers.into_iter().flat_map(|w| w.join().unwrap_or_default()).collect()
    });
    files.sort_by(|a, b| a.path.cmp(&b.path));
    TextResults { files, capped: capped.load(Ordering::Relaxed) }
}

fn line_hit(matcher: &RegexMatcher, line: u64, text: &str) -> LineHit {
    let text = text.trim_end_matches(['\n', '\r']).replace('\t', "    ");
    let text = text.trim_start();
    let mut lit = Vec::new();
    let _ = matcher.find_iter(text.as_bytes(), |m| {
        lit.push(m.start()..m.end());
        true
    });
    // A long line is cut around its first match, at character boundaries.
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
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cc-gpui-search-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn index_skips_ignored_and_git() {
        let root = temp("index");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("target")).unwrap();
        std::fs::write(root.join(".gitignore"), "target/\n").unwrap();
        std::fs::write(root.join(".git/HEAD"), "x").unwrap();
        std::fs::write(root.join("target/out"), "x").unwrap();
        std::fs::write(root.join("src/a.rs"), "x").unwrap();
        std::fs::write(root.join("README"), "x").unwrap();
        let paths = index(&root, &AtomicBool::new(false));
        assert_eq!(paths, vec![".gitignore", "README", "src/a.rs"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn names_prefer_the_named_file() {
        let paths: Vec<String> = [
            "src/relay/workspace-session-handler.ts",
            "src/renderer/src/lib/workspace-session.ts",
            "docs/other.md",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let hits = names(&paths, "workspace-session");
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].path, "src/renderer/src/lib/workspace-session.ts");
        // The lit letters spell the query.
        let lit: String = hits[0].lit.iter().map(|r| &hits[0].path[r.clone()]).collect();
        assert_eq!(lit, "workspace-session");
        assert!(names(&paths, "  ").is_empty());
    }

    #[test]
    fn char_ranges_merge_and_respect_utf8() {
        assert_eq!(char_ranges("aéb", &[1, 2]), vec![1..4]);
        assert_eq!(char_ranges("abc", &[0, 2]), vec![0..1, 2..3]);
    }

    #[test]
    fn text_search_is_literal_smart_case_and_skips_binary() {
        let root = temp("text");
        std::fs::write(root.join("a.txt"), "one Foo\n\tfoo(bar)\nnone\n").unwrap();
        std::fs::write(root.join("b.bin"), b"foo\0foo").unwrap();
        std::fs::write(root.join("c.txt"), "FOO\n").unwrap();
        let paths = vec!["a.txt".to_string(), "b.bin".to_string(), "c.txt".to_string()];
        let cancel = AtomicBool::new(false);

        let found = text(&root, &paths, "foo(", &cancel);
        assert_eq!(found.files.len(), 1);
        assert_eq!(found.files[0].lines, vec![LineHit { line: 2, text: "foo(bar)".into(), lit: vec![0..4] }]);

        // Lower case matches any case; upper case only itself.
        let found = text(&root, &paths, "foo", &cancel);
        assert_eq!(found.files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), vec!["a.txt", "c.txt"]);
        assert_eq!(found.count(), 3);
        assert_eq!(text(&root, &paths, "Foo", &cancel).count(), 1);
        assert!(text(&root, &paths, " ", &cancel).files.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn text_search_caps_matches() {
        let root = temp("cap");
        std::fs::write(root.join("many.txt"), "x\n".repeat(MAX_MATCHES + 5)).unwrap();
        let found = text(&root, &["many.txt".to_string()], "x", &AtomicBool::new(false));
        assert_eq!(found.count(), MAX_MATCHES);
        assert!(found.capped);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn long_lines_are_cut_around_the_match() {
        let matcher = matcher("needle").unwrap();
        let line = format!("{}needle{}", "a".repeat(1000), "b".repeat(1000));
        let hit = line_hit(&matcher, 1, &line);
        assert!(hit.text.starts_with('…') && hit.text.ends_with('…'));
        assert_eq!(&hit.text[hit.lit[0].clone()], "needle");
    }
}
