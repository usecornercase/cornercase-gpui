use std::collections::HashMap;
use std::ops::Range;

use super::wire::{FileInfo, FileStatus, Gap, Hunk, LineKind, Segment};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Row {
    Gap { hunk: usize, lines: u32 },
    GapLine { hunk: usize, line: usize },
    Hunk(usize),
    Line { hunk: usize, line: usize },
}

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

fn new_end(hunk: &Hunk) -> u32 {
    hunk.new_start.saturating_add(count(hunk, LineKind::Removed))
}

pub(super) fn gap_before(hunks: &[Hunk], i: usize) -> u32 {
    let Some(hunk) = hunks.get(i) else { return 0 };
    if hunk.new_start == 0 {
        return 0;
    }
    let from = if i == 0 { 1 } else { new_end(&hunks[i - 1]) };
    hunk.new_start.saturating_sub(from)
}

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

pub(super) fn reference(path: &str, (first, last): (u32, u32)) -> String {
    if first == last { format!("{path}:{first} ") } else { format!("{path}:{first}-{last} ") }
}

pub(super) fn editor_command(path: &str, line: u32) -> String {
    format!("${{VISUAL:-${{EDITOR:-vi}}}} +{line} {}", shell_quote(path))
}

fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Run {
    pub range: Range<usize>,
    pub style: usize,
    pub emphasis: bool,
}

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
mod tests;
