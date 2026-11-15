use std::collections::HashSet;
use std::ops::Range;

use super::{Kind, Line};

pub(crate) const STEPS_SHOWN: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Entry {
    You { id: u64, text: String, at: i64, held: bool },
    Answer { id: u64, text: String, at: i64 },
    Steps { first: u64, texts: Vec<String> },
    Failed { id: u64, text: String },
    Notice { id: u64, text: String },
    Other { id: u64, text: String },
}

impl Entry {
    pub(crate) fn id(&self) -> u64 {
        match self {
            Self::You { id, .. }
            | Self::Answer { id, .. }
            | Self::Failed { id, .. }
            | Self::Notice { id, .. }
            | Self::Other { id, .. } => *id,
            Self::Steps { first, .. } => *first,
        }
    }
}

const DROPPED: [&str; 3] = ["stopped", "interrupted by a restart", "1 held message dropped"];

fn ends_a_turn(line: &Line) -> bool {
    match line.kind {
        Kind::Bot | Kind::Error => true,
        Kind::Notice => DROPPED.contains(&line.text.as_str()) || line.text.ends_with(" held messages dropped"),
        _ => false,
    }
}

fn after_the_last_end(lines: &[Line]) -> &[Line] {
    let from = lines.iter().rposition(ends_a_turn).map_or(0, |at| at + 1);
    &lines[from..]
}

pub(crate) fn held(lines: &[Line]) -> HashSet<u64> {
    after_the_last_end(lines).iter().filter(|line| line.kind == Kind::You && line.held).map(|line| line.id).collect()
}

pub(crate) fn entries(lines: &[Line]) -> Vec<Entry> {
    let held = held(lines);
    let mut out: Vec<Entry> = Vec::new();
    for line in lines {
        if let (Kind::Step, Some(Entry::Steps { texts, .. })) = (&line.kind, out.last_mut()) {
            texts.push(line.text.clone());
            continue;
        }
        out.push(entry_of(line, held.contains(&line.id)));
    }
    out
}

fn entry_of(line: &Line, held: bool) -> Entry {
    let (id, text, at) = (line.id, line.text.clone(), line.at);
    match &line.kind {
        Kind::You => Entry::You { id, text, at, held },
        Kind::Bot => Entry::Answer { id, text, at },
        Kind::Step => Entry::Steps { first: id, texts: vec![text] },
        Kind::Error => Entry::Failed { id, text },
        Kind::Notice => Entry::Notice { id, text },
        Kind::Other(_) => Entry::Other { id, text },
    }
}

pub(crate) fn folded(texts: &[String], open: bool) -> (usize, &[String]) {
    if open || texts.len() <= STEPS_SHOWN {
        return (0, texts);
    }
    let hidden = texts.len() - STEPS_SHOWN;
    (hidden, &texts[hidden..])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Turn {
    pub since: i64,
    pub steps: usize,
}

pub(crate) fn turn(lines: &[Line], thinking: bool) -> Option<Turn> {
    if !thinking {
        return None;
    }
    let tail = after_the_last_end(lines);
    let ended = lines.len() - tail.len();
    let since = tail
        .iter()
        .find(|line| line.kind == Kind::You && !line.held)
        .map(|line| line.at)
        .or_else(|| ended.checked_sub(1).map(|at| lines[at].at))
        .or_else(|| lines.iter().find(|line| line.kind == Kind::You).map(|line| line.at))?;
    Some(Turn { since, steps: tail.iter().filter(|line| line.kind == Kind::Step).count() })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EntryKey {
    kind: u8,
    first: u64,
    last: u64,
    held: bool,
    open: bool,
}

pub(crate) fn earlier_key() -> EntryKey {
    EntryKey { kind: 9, first: 0, last: 0, held: false, open: false }
}

pub(crate) fn keys(entries: &[Entry], opened: &HashSet<u64>) -> Vec<EntryKey> {
    entries
        .iter()
        .map(|entry| {
            let (kind, last, held) = match entry {
                Entry::You { id, held, .. } => (0, *id, *held),
                Entry::Answer { id, .. } => (1, *id, false),
                Entry::Steps { first, texts } => (2, *first + texts.len() as u64, false),
                Entry::Failed { id, .. } => (3, *id, false),
                Entry::Notice { id, .. } => (4, *id, false),
                Entry::Other { id, .. } => (5, *id, false),
            };
            EntryKey { kind, first: entry.id(), last, held, open: opened.contains(&entry.id()) }
        })
        .collect()
}

pub(crate) fn spliced(old: &[EntryKey], new: &[EntryKey]) -> Vec<(Range<usize>, usize)> {
    if old == new {
        return Vec::new();
    }
    let dropped = (1..old.len()).find(|&d| new.starts_with(&old[d..]));
    if let (true, Some(d)) = (old.first() != new.first(), dropped) {
        let mut out = vec![(0..d, 0)];
        let kept = old.len() - d;
        if new.len() > kept {
            out.push((kept..kept, new.len() - kept));
        }
        return out;
    }
    let prefix = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    let suffix = old[prefix..].iter().rev().zip(new[prefix..].iter().rev()).take_while(|(a, b)| a == b).count();
    vec![(prefix..old.len() - suffix, new.len() - suffix - prefix)]
}
