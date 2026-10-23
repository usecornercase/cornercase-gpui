//! The pure part of drawing a patch: the rows the diff list shows, each line's coloured runs, a
//! hunk as a patch to copy, the `path:a-b` reference an agent reads, and the texts of the actions.

use std::collections::HashMap;
use std::ops::Range;

use super::wire::{FileInfo, FileStatus, Gap, Hunk, LineKind, Segment};

/// One row of the diff list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Row {
    /// The unchanged lines before hunk `hunk` (from 0), not read yet: a button that reads them.
    Gap {
        hunk: usize,
        lines: u32,
    },
    /// One of those lines, once read.
    GapLine {
        hunk: usize,
        line: usize,
    },
    /// A hunk's `@@ … @@` header and its actions.
    Hunk(usize),
    Line {
        hunk: usize,
        line: usize,
    },
}

/// The rows of a patch, with the gaps that were read shown in full. Only gaps between two hunks
/// can be read, as in the TUI: `changes gap` has none above the first or below the last.
pub(super) fn rows(hunks: &[Hunk], gaps: &HashMap<usize, Gap>) -> Vec<Row> {
    let mut rows = Vec::new();
    for (i, hunk) in hunks.iter().enumerate() {
        let before = gap_before(hunks, i);
        match gaps.get(&i) {
            Some(gap) => rows.extend((0..gap.lines.len()).map(|line| Row::GapLine { hunk: i, line })),
            None if i > 0 && before > 0 => rows.push(Row::Gap { hunk: i, lines: before }),
            None => {}
        }
        rows.push(Row::Hunk(i));
        rows.extend((0..hunk.lines.len()).map(|line| Row::Line { hunk: i, line }));
    }
    rows
}

fn count(hunk: &Hunk, skip: LineKind) -> u32 {
    u32::try_from(hunk.lines.iter().filter(|l| l.kind != skip).count()).unwrap_or(u32::MAX)
}

/// The first line of the new side after the hunk.
fn new_end(hunk: &Hunk) -> u32 {
    hunk.new_start.saturating_add(count(hunk, LineKind::Removed))
}

/// How many unchanged lines sit between hunk `i` and the one before it (or the top of the file).
pub(super) fn gap_before(hunks: &[Hunk], i: usize) -> u32 {
    let Some(hunk) = hunks.get(i) else { return 0 };
    if hunk.new_start == 0 {
        return 0;
    }
    let from = if i == 0 { 1 } else { new_end(&hunks[i - 1]) };
    hunk.new_start.saturating_sub(from)
}

/// The `@@ -a,b +c,d @@ context` header of a hunk.
pub(super) fn header(hunk: &Hunk) -> String {
    let mut text = format!(
        "@@ -{},{} +{},{} @@",
        hunk.old_start,
        count(hunk, LineKind::Added),
        hunk.new_start,
        count(hunk, LineKind::Removed)
    );
    if !hunk.context.is_empty() {
        text.push(' ');
        text.push_str(&hunk.context);
    }
    text
}

/// The hunk as a patch, as the TUI's hunk copy gives it.
pub(super) fn patch(hunk: &Hunk) -> String {
    let mut text = header(hunk);
    for line in &hunk.lines {
        text.push('\n');
        text.push(match line.kind {
            LineKind::Context => ' ',
            LineKind::Removed => '-',
            LineKind::Added => '+',
        });
        text.push_str(&line.text);
    }
    text.push('\n');
    text
}

/// The new side's lines the hunk changed: its added lines, or the line where lines were removed.
pub(super) fn changed(hunk: &Hunk) -> (u32, u32) {
    let added: Vec<u32> = hunk.lines.iter().filter(|l| l.kind == LineKind::Added).filter_map(|l| l.new).collect();
    if let (Some(first), Some(last)) = (added.first(), added.last()) {
        return (*first, *last);
    }
    let mut new = hunk.new_start.max(1);
    for line in &hunk.lines {
        match line.kind {
            LineKind::Removed => break,
            LineKind::Context | LineKind::Added => new = line.new.map_or(new, |n| n + 1),
        }
    }
    (new, new)
}

/// `path:a-b ` (or `path:a `), what an agent reads as lines of the file as it is now.
pub(super) fn reference(path: &str, (first, last): (u32, u32)) -> String {
    if first == last { format!("{path}:{first} ") } else { format!("{path}:{first}-{last} ") }
}

/// The line typed into a new tab to open the file in the user's editor at a line, as the TUI's
/// hunk `open` runs it.
pub(super) fn editor_command(path: &str, line: u32) -> String {
    format!("${{VISUAL:-${{EDITOR:-vi}}}} +{line} {}", shell_quote(path))
}

fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// What the discard confirmation asks, as cornercase's own words for each kind of change.
pub(super) fn discard_question(file: &FileInfo) -> String {
    let path = &file.path;
    match (file.status, file.old_path.as_deref()) {
        (FileStatus::Untracked, _) => format!("Delete {path}? It is not in git, so this cannot be undone."),
        (FileStatus::Added, _) => {
            format!("Discard the changes to {path}? It is new, so it is deleted. This cannot be undone.")
        }
        (FileStatus::Renamed, Some(old)) => format!(
            "Discard the changes to {old} → {path}? {old} comes back as it was in the last commit and {path} is deleted. \
             This cannot be undone."
        ),
        _ => format!("Discard the changes to {path}? This cannot be undone."),
    }
}

/// A piece of a line drawn in one style: a byte range of the text, the patch's style index and
/// whether it is a word that changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Run {
    pub range: Range<usize>,
    pub style: usize,
    pub emphasis: bool,
}

/// Splits a line into runs: the highlighting's segments (style 0 where there is none), cut again
/// where the emphasised words (character indices) start and end. The runs cover the text, in
/// order and without overlapping; segments that do not add up to the text are ignored.
pub(super) fn runs(text: &str, syntax: Option<&[Segment]>, emphasis: &[[usize; 2]]) -> Vec<Run> {
    let mut styled: Vec<(Range<usize>, usize)> = Vec::new();
    let mut at = 0;
    for seg in syntax.unwrap_or_default() {
        let end = at + seg.text.len();
        if text.get(at..end) != Some(seg.text.as_str()) {
            styled.clear();
            at = 0;
            break;
        }
        if end > at {
            styled.push((at..end, seg.style));
        }
        at = end;
    }
    if at < text.len() {
        styled.push((at..text.len(), 0));
    }
    let bytes = |char_index: usize| text.char_indices().nth(char_index).map_or(text.len(), |(b, _)| b);
    let marks: Vec<Range<usize>> =
        emphasis.iter().map(|[a, b]| bytes(*a)..bytes(*b)).filter(|r| r.start < r.end).collect();
    let mut cuts: Vec<usize> = marks.iter().flat_map(|r| [r.start, r.end]).collect();
    cuts.sort_unstable();
    let mut out = Vec::new();
    for (range, style) in styled {
        let mut start = range.start;
        let inner = cuts.iter().copied().filter(|c| *c > range.start && *c < range.end);
        for end in inner.chain(std::iter::once(range.end)) {
            if end > start {
                let emphasis = marks.iter().any(|m| m.start <= start && end <= m.end);
                out.push(Run { range: start..end, style, emphasis });
                start = end;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panels::changes::wire::{GapLine, Line};

    fn line(kind: LineKind, old: Option<u32>, new: Option<u32>, text: &str) -> Line {
        Line { kind, old, new, text: text.into(), ..Line::default() }
    }

    fn hunk(old_start: u32, new_start: u32, lines: Vec<Line>) -> Hunk {
        Hunk { old_start, new_start, context: String::new(), lines }
    }

    fn sample() -> Vec<Hunk> {
        vec![
            hunk(
                3,
                3,
                vec![
                    line(LineKind::Context, Some(3), Some(3), "a"),
                    line(LineKind::Removed, Some(4), None, "b"),
                    line(LineKind::Added, None, Some(4), "B"),
                    line(LineKind::Added, None, Some(5), "C"),
                    line(LineKind::Context, Some(5), Some(6), "d"),
                ],
            ),
            hunk(
                10,
                11,
                vec![line(LineKind::Context, Some(10), Some(11), "x"), line(LineKind::Removed, Some(11), None, "y")],
            ),
        ]
    }

    #[test]
    fn counts_the_unchanged_lines_between_hunks() {
        let hunks = sample();
        assert_eq!(gap_before(&hunks, 0), 2);
        // The first hunk ends before new line 7; the second starts at 11.
        assert_eq!(gap_before(&hunks, 1), 4);
        assert_eq!(gap_before(&hunks, 2), 0);
        // A deleted file's hunk starts at 0 on the new side.
        assert_eq!(gap_before(&[hunk(1, 0, vec![line(LineKind::Removed, Some(1), None, "z")])], 0), 0);
    }

    #[test]
    fn lists_rows_with_gaps_folded_or_read() {
        let hunks = sample();
        let mut gaps = HashMap::new();
        let rows_folded = rows(&hunks, &gaps);
        // No row for the 2 lines above the first hunk: they cannot be read.
        assert_eq!(rows_folded[0], Row::Hunk(0));
        assert_eq!(rows_folded[6], Row::Gap { hunk: 1, lines: 4 });
        assert_eq!(rows_folded.len(), 1 + 5 + 1 + 1 + 2);
        let gap_line = |n| GapLine { old: n, new: n, text: "u".into(), syntax: None };
        gaps.insert(1, Gap { lines: vec![gap_line(7), gap_line(8)], ..Gap::default() });
        let rows_read = rows(&hunks, &gaps);
        assert_eq!(
            &rows_read[6..9],
            &[Row::GapLine { hunk: 1, line: 0 }, Row::GapLine { hunk: 1, line: 1 }, Row::Hunk(1)]
        );
    }

    #[test]
    fn writes_a_hunk_as_a_patch() {
        let mut h = sample().remove(0);
        h.context = "fn main()".into();
        assert_eq!(header(&h), "@@ -3,3 +3,4 @@ fn main()");
        assert_eq!(patch(&h), "@@ -3,3 +3,4 @@ fn main()\n a\n-b\n+B\n+C\n d\n");
    }

    #[test]
    fn references_the_changed_lines() {
        let hunks = sample();
        assert_eq!(changed(&hunks[0]), (4, 5));
        // Only removed: the line where they were removed.
        assert_eq!(changed(&hunks[1]), (12, 12));
        assert_eq!(reference("src/a.rs", (4, 5)), "src/a.rs:4-5 ");
        assert_eq!(reference("src/a.rs", (12, 12)), "src/a.rs:12 ");
    }

    #[test]
    fn quotes_the_path_for_the_editor() {
        assert_eq!(editor_command("src/a b.rs", 7), "${VISUAL:-${EDITOR:-vi}} +7 'src/a b.rs'");
        assert_eq!(editor_command("it's.txt", 1), r"${VISUAL:-${EDITOR:-vi}} +1 'it'\''s.txt'");
    }

    #[test]
    fn asks_in_cornercase_words() {
        let file = |status, old: Option<&str>| FileInfo {
            path: "new.rs".into(),
            old_path: old.map(Into::into),
            status,
            ..FileInfo::default()
        };
        assert!(discard_question(&file(FileStatus::Untracked, None)).starts_with("Delete new.rs?"));
        assert!(discard_question(&file(FileStatus::Added, None)).contains("It is new, so it is deleted"));
        assert!(discard_question(&file(FileStatus::Renamed, Some("old.rs"))).contains("old.rs → new.rs"));
        assert_eq!(
            discard_question(&file(FileStatus::Modified, None)),
            "Discard the changes to new.rs? This cannot be undone."
        );
    }

    fn seg(style: usize, text: &str) -> Segment {
        Segment { style, text: text.into() }
    }

    #[test]
    fn runs_follow_the_highlighting() {
        let text = "let x = 1;";
        let syntax = [seg(1, "let"), seg(0, " x = "), seg(4, "1"), seg(0, ";")];
        let r = runs(text, Some(&syntax), &[]);
        assert_eq!(r.len(), 4);
        assert_eq!((r[0].range.clone(), r[0].style), (0..3, 1));
        assert_eq!((r[2].range.clone(), r[2].style), (8..9, 4));
        assert!(r.iter().all(|r| !r.emphasis));
    }

    #[test]
    fn runs_are_cut_by_the_emphasis() {
        // "    let y = 2;" with y and 2 emphasised (characters 8..9 and 12..13).
        let text = "    let y = 2;";
        let syntax = [seg(0, "    "), seg(1, "let"), seg(0, " y = "), seg(4, "2"), seg(0, ";")];
        let r = runs(text, Some(&syntax), &[[8, 9], [12, 13]]);
        let emphasised: Vec<_> = r.iter().filter(|r| r.emphasis).map(|r| &text[r.range.clone()]).collect();
        assert_eq!(emphasised, vec!["y", "2"]);
        let joined: String = r.iter().map(|r| &text[r.range.clone()]).collect();
        assert_eq!(joined, text);
        assert!(r.windows(2).all(|w| w[0].range.end == w[1].range.start));
    }

    #[test]
    fn runs_count_characters_not_bytes() {
        let text = "é → ü";
        let r = runs(text, None, &[[4, 5]]);
        let emphasised: Vec<_> = r.iter().filter(|r| r.emphasis).map(|r| &text[r.range.clone()]).collect();
        assert_eq!(emphasised, vec!["ü"]);
        assert_eq!(r.first().map(|r| r.style), Some(0));
    }

    #[test]
    fn runs_ignore_highlighting_that_does_not_match() {
        let r = runs("abc", Some(&[seg(2, "xyz")]), &[]);
        assert_eq!(r, vec![Run { range: 0..3, style: 0, emphasis: false }]);
        assert!(runs("", None, &[]).is_empty());
    }
}
