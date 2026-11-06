use std::collections::HashSet;
use std::ops::Range;

use serde::Deserialize;

use crate::panels::changes::FileStatus;

const WHOLE_FILE: Range<usize> = 0..usize::MAX;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Mark {
    Added,
    Changed,
    Removed,
    RemovedAbove,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Block {
    pub lines: Range<usize>,
    pub mark: Mark,
    pub before: usize,
    pub old: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Gutter {
    blocks: Vec<Block>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Kind {
    Removed,
    Added,
    #[default]
    #[serde(other)]
    Context,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Line {
    kind: Kind,
    new: Option<usize>,
    text: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Hunk {
    new_start: usize,
    lines: Vec<Line>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Changed {
    status: FileStatus,
    hunks: Vec<Hunk>,
}

fn block_of(old: Vec<String>, added: &[usize], next: usize) -> Block {
    if let (Some(first), Some(last)) = (added.first(), added.last()) {
        let mark = if old.is_empty() { Mark::Added } else { Mark::Changed };
        return Block { lines: *first..*last + 1, mark, before: *first, old };
    }
    match next.checked_sub(1) {
        Some(line) => Block { lines: line..next, mark: Mark::Removed, before: next, old },
        None => Block { lines: 0..1, mark: Mark::RemovedAbove, before: 0, old },
    }
}

fn blocks_of(hunk: &Hunk) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut next = hunk.new_start.saturating_sub(1);
    let mut lines = hunk.lines.iter().peekable();
    while let Some(line) = lines.peek() {
        if line.kind == Kind::Context {
            next = line.new.unwrap_or(next + 1);
            lines.next();
            continue;
        }
        let mut old = Vec::new();
        while let Some(line) = lines.next_if(|line| line.kind == Kind::Removed) {
            old.push(line.text.clone());
        }
        let mut added = Vec::new();
        while let Some(line) = lines.next_if(|line| line.kind == Kind::Added) {
            added.extend(line.new.and_then(|new| new.checked_sub(1)));
        }
        blocks.push(block_of(old, &added, next));
        next = added.last().map_or(next, |last| last + 1);
    }
    blocks
}

impl Gutter {
    pub(super) fn parse(json: &str) -> Option<Self> {
        let changed = serde_json::from_str::<Changed>(json).ok()?;
        if matches!(changed.status, FileStatus::Added | FileStatus::Untracked) {
            return Some(Self {
                blocks: vec![Block { lines: WHOLE_FILE, mark: Mark::Added, before: 0, old: Vec::new() }],
            });
        }
        Some(Self { blocks: changed.hunks.iter().flat_map(blocks_of).collect() })
    }

    pub(super) fn at(&self, line: usize) -> Option<&Block> {
        let at = self.blocks.partition_point(|block| block.lines.end <= line);
        self.blocks.get(at).filter(|block| block.lines.contains(&line))
    }

    pub(super) fn before(&self, line: usize) -> Option<&Block> {
        self.blocks.iter().find(|block| block.before == line && !block.old.is_empty())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Follow {
    Nothing,
    Cleared,
    Read(u64),
}

#[derive(Debug, Default)]
pub(super) struct Margin {
    pub gutter: Option<Gutter>,
    pub unfolded: HashSet<usize>,
    status: Option<FileStatus>,
    asked: u64,
}

impl Margin {
    pub(super) fn follow(&mut self, status: Option<FileStatus>, again: bool) -> Follow {
        let changed = self.status != status;
        self.status = status;
        if status.is_none() {
            self.asked += 1;
            self.unfolded.clear();
            return if self.gutter.take().is_some() { Follow::Cleared } else { Follow::Nothing };
        }
        if !changed && !again {
            return Follow::Nothing;
        }
        self.asked += 1;
        Follow::Read(self.asked)
    }

    pub(super) fn answered(&mut self, asked: u64, gutter: Option<Gutter>) -> bool {
        if asked != self.asked || self.gutter == gutter {
            return false;
        }
        self.gutter = gutter;
        let gutter = self.gutter.as_ref();
        self.unfolded.retain(|before| gutter.is_some_and(|gutter| gutter.before(*before).is_some()));
        true
    }

    pub(super) fn toggle(&mut self, before: usize) {
        if !self.unfolded.remove(&before) {
            self.unfolded.insert(before);
        }
    }

    pub(super) fn unfolded_before(&self, line: usize) -> Option<&Block> {
        self.unfolded.contains(&line).then(|| self.gutter.as_ref()?.before(line)).flatten()
    }
}

#[cfg(test)]
mod tests;
