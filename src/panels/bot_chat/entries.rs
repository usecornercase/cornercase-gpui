use std::collections::HashSet;
use std::ops::Range;

use crate::model::bots::{Kind, Line};

pub(crate) const STEPS_SHOWN: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Entry {
    You { id: u64, text: String, at: i64, held: bool },
    Answer { id: u64, text: String, at: i64 },
    Steps { first: u64, last: u64, texts: Vec<String> },
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
        if let (Kind::Step, Some(Entry::Steps { texts, last, .. })) = (&line.kind, out.last_mut()) {
            texts.push(line.text.clone());
            *last = line.id;
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
        Kind::Step => Entry::Steps { first: id, last: id, texts: vec![text] },
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

pub(crate) fn keys(entries: &[Entry], opened: &HashSet<u64>) -> Vec<EntryKey> {
    entries
        .iter()
        .map(|entry| {
            let (kind, last, held) = match entry {
                Entry::You { id, held, .. } => (0, *id, *held),
                Entry::Answer { id, .. } => (1, *id, false),
                Entry::Steps { last, .. } => (2, *last, false),
                Entry::Failed { id, .. } => (3, *id, false),
                Entry::Notice { id, .. } => (4, *id, false),
                Entry::Other { id, .. } => (5, *id, false),
            };
            EntryKey { kind, first: entry.id(), last, held, open: opened.contains(&entry.id()) }
        })
        .collect()
}

impl EntryKey {
    fn overlaps(&self, other: &Self) -> bool {
        self.first <= other.last && other.first <= self.last
    }
}

pub(crate) type Splices = Vec<(Range<usize>, usize)>;

pub(crate) fn spliced(old: &[EntryKey], new: &[EntryKey]) -> Splices {
    if old == new {
        return Vec::new();
    }
    let dropped =
        new.first().map_or(old.len(), |first| old.iter().position(|key| key.overlaps(first)).unwrap_or(old.len()));
    let kept = &old[dropped..];
    let mut out: Splices = Vec::new();
    if dropped > 0 {
        out.push((0..dropped, 0));
    }
    let common = kept.len().min(new.len());
    let mut at = 0;
    while at < common {
        if kept[at] == new[at] {
            at += 1;
            continue;
        }
        let start = at;
        while at < common && kept[at] != new[at] {
            at += 1;
        }
        out.push((start..at, at - start));
    }
    match new.len().cmp(&kept.len()) {
        std::cmp::Ordering::Greater => out.push((kept.len()..kept.len(), new.len() - kept.len())),
        std::cmp::Ordering::Less => out.push((new.len()..kept.len(), 0)),
        std::cmp::Ordering::Equal => {}
    }
    out
}

pub(crate) fn row_splices(old: (bool, &[EntryKey]), new: (bool, &[EntryKey])) -> Splices {
    let (had, now) = (old.0, new.0);
    let note = match (had, now) {
        (false, true) => vec![(0..0, 1)],
        (true, false) => vec![(0..1, 0)],
        _ => Vec::new(),
    };
    let shift = usize::from(now);
    let entries =
        spliced(old.1, new.1).into_iter().map(|(range, count)| (range.start + shift..range.end + shift, count));
    note.into_iter().chain(entries).collect()
}
