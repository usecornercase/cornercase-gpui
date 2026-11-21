use std::collections::{HashMap, HashSet};
use std::ops::Range;

use super::runs::{block, offers_again};
use crate::model::bots::{Kind, Line, Runs};

pub(crate) const STEPS_SHOWN: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Entry {
    You { id: u64, text: String, at: i64, held: bool, dropped: bool },
    Answer { id: u64, text: String, at: i64 },
    Steps { first: u64, last: u64, texts: Vec<String> },
    Failed { id: u64, text: String },
    Notice { id: u64, text: String, ask: Option<u64> },
    Other { id: u64, text: String },
    Run(Block),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Block {
    pub run: u64,
    pub head: u64,
    pub tail: u64,
    pub title: String,
    pub steps: Vec<String>,
    pub answer: Option<(u64, String, i64)>,
    pub error: Option<String>,
    pub end: Option<String>,
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
            Self::Run(block) => block.head,
        }
    }

    pub(crate) fn run(&self) -> Option<u64> {
        match self {
            Self::Run(block) => Some(block.run),
            _ => None,
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

const SETTING_UP: &str = "setting up";

fn is_setup(line: &Line) -> bool {
    line.kind == Kind::Notice && line.text == SETTING_UP
}

fn opens_a_turn(line: &Line) -> bool {
    (line.kind == Kind::You && !line.held) || is_setup(line)
}

fn own(lines: &[Line]) -> Vec<&Line> {
    lines.iter().filter(|line| line.routine.is_none()).collect()
}

fn after_the_last_end<'a>(lines: &'a [&'a Line]) -> &'a [&'a Line] {
    let from = lines.iter().rposition(|line| ends_a_turn(line)).map_or(0, |at| at + 1);
    &lines[from..]
}

pub(crate) fn held(lines: &[Line]) -> HashSet<u64> {
    let lines = own(lines);
    after_the_last_end(&lines).iter().filter(|line| line.kind == Kind::You && line.held).map(|line| line.id).collect()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Asked {
    pub ended: Vec<u64>,
    pub dropped: HashSet<u64>,
}

const ENDS: [&str; 2] = ["stopped", "interrupted by a restart"];

fn dropped_in(text: &str) -> usize {
    let count = text.strip_suffix(" held message dropped").or_else(|| text.strip_suffix(" held messages dropped"));
    count.and_then(|count| count.parse().ok()).unwrap_or_default()
}

#[derive(Default)]
struct Walk {
    turn: Vec<u64>,
    waiting: Vec<u64>,
    closing: bool,
    asked: Asked,
}

impl Walk {
    fn drop_first(&mut self, count: usize) {
        let count = count.min(self.waiting.len());
        self.asked.dropped.extend(self.waiting.drain(..count));
    }

    fn end(&mut self) {
        self.asked.ended = std::mem::take(&mut self.turn);
        self.closing = true;
    }

    fn step(&mut self, line: &Line) {
        let dropped = if line.kind == Kind::Notice { dropped_in(&line.text) } else { 0 };
        if dropped > 0 {
            self.drop_first(dropped);
            return;
        }
        if std::mem::take(&mut self.closing) {
            self.turn = std::mem::take(&mut self.waiting);
        }
        match line.kind {
            Kind::You if line.held => self.waiting.push(line.id),
            _ if opens_a_turn(line) => self.turn = vec![line.id],
            Kind::Bot | Kind::Error => self.end(),
            Kind::Notice if ENDS.contains(&line.text.as_str()) => self.end(),
            _ => {}
        }
    }
}

pub(crate) fn asked(lines: &[Line]) -> Asked {
    let mut walk = Walk::default();
    for line in own(lines) {
        walk.step(line);
    }
    walk.asked
}

pub(crate) fn entries(lines: &[Line]) -> Vec<Entry> {
    let held = held(lines);
    let dropped = asked(lines).dropped;
    let mut out: Vec<Entry> = Vec::new();
    let mut runs: HashMap<u64, (usize, Vec<&Line>)> = HashMap::new();
    for line in lines {
        if let Some(run) = line.routine {
            let (_, members) = runs.entry(run).or_insert_with(|| {
                out.push(block(run, &[line]));
                (out.len() - 1, Vec::new())
            });
            members.push(line);
            continue;
        }
        if let (Kind::Step, Some(Entry::Steps { texts, last, .. })) = (&line.kind, out.last_mut()) {
            texts.push(line.text.clone());
            *last = line.id;
            continue;
        }
        out.push(entry_of(line, (held.contains(&line.id), dropped.contains(&line.id))));
    }
    for (run, (at, members)) in runs {
        out[at] = block(run, &members);
    }
    out
}

fn entry_of(line: &Line, (held, dropped): (bool, bool)) -> Entry {
    let (id, text, at) = (line.id, line.text.clone(), line.at);
    match &line.kind {
        Kind::You => Entry::You { id, text, at, held, dropped },
        Kind::Bot => Entry::Answer { id, text, at },
        Kind::Step => Entry::Steps { first: id, last: id, texts: vec![text] },
        Kind::Error => Entry::Failed { id, text },
        Kind::Notice => Entry::Notice { id, text, ask: line.ask },
        Kind::Other(_) => Entry::Other { id, text },
    }
}

pub(crate) fn folded(texts: &[String], open: bool, kept: usize) -> (usize, &[String]) {
    if open || texts.len() <= kept {
        return (0, texts);
    }
    let hidden = texts.len() - kept;
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
    let lines = own(lines);
    let tail = after_the_last_end(&lines);
    let ended = lines.len() - tail.len();
    let since = tail
        .iter()
        .find(|line| opens_a_turn(line))
        .map(|line| line.at)
        .or_else(|| ended.checked_sub(1).map(|at| lines[at].at))
        .or_else(|| lines.iter().find(|line| line.kind == Kind::You).map(|line| line.at))?;
    Some(Turn { since, steps: tail.iter().filter(|line| line.kind == Kind::Step).count() })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Again {
    Say(String),
    Setup,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Retry {
    pub error: u64,
    pub again: Again,
}

fn again_of(asked: &[&Line], setup: bool) -> Option<Again> {
    match asked {
        [] => None,
        [only] if is_setup(only) => setup.then_some(Again::Setup),
        _ => {
            let texts: Vec<&str> = asked.iter().map(|line| line.text.as_str()).collect();
            Some(Again::Say(texts.join("\n\n")))
        }
    }
}

pub(crate) fn retry(lines: &[Line], thinking: bool, setup: bool) -> Option<Retry> {
    let last = own(lines).last().copied().filter(|line| !thinking && line.kind == Kind::Error)?;
    let ended = asked(lines).ended;
    let asked: Vec<&Line> = ended.iter().filter_map(|id| lines.iter().find(|line| line.id == *id)).collect();
    again_of(&asked, setup).map(|again| Retry { error: last.id, again })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Control {
    Ready,
    Running,
    Resting,
}

pub(crate) fn send_control(blank: bool, on_its_way: bool) -> Control {
    match (on_its_way, blank) {
        (true, _) => Control::Running,
        (false, true) => Control::Resting,
        (false, false) => Control::Ready,
    }
}

pub(crate) fn stop_control(stopping: bool) -> Control {
    if stopping { Control::Running } else { Control::Ready }
}

pub(crate) fn new_conversation_control(thinking: bool, starting: bool) -> Control {
    match (starting, thinking) {
        (true, _) => Control::Running,
        (false, true) => Control::Resting,
        (false, false) => Control::Ready,
    }
}

pub(crate) fn givable(lines: &[Line], waiting: &[u64]) -> HashMap<u64, u64> {
    waiting
        .iter()
        .filter_map(|ask| lines.iter().find(|line| line.ask == Some(*ask)).map(|line| (line.id, *ask)))
        .collect()
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Give {
    #[default]
    No,
    Button,
    Needs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EntryKey {
    kind: u8,
    first: u64,
    last: u64,
    tail: u64,
    held: bool,
    open: bool,
    retry: bool,
    give: Give,
}

fn block_key(block: &Block, opened: &HashSet<u64>, runs: &Runs) -> EntryKey {
    EntryKey {
        kind: 7,
        first: block.head,
        last: block.head,
        tail: block.tail,
        held: runs.answers(block.run),
        open: opened.contains(&block.head),
        retry: offers_again(block, runs),
        give: Give::No,
    }
}

pub(crate) fn keys(
    entries: &[Entry],
    opened: &HashSet<u64>,
    retry: Option<u64>,
    (givable, give): (&HashMap<u64, u64>, Give),
    runs: &Runs,
) -> Vec<EntryKey> {
    entries
        .iter()
        .map(|entry| {
            let (kind, last, held) = match entry {
                Entry::Run(block) => return block_key(block, opened, runs),
                Entry::You { id, dropped: true, .. } => (6, *id, false),
                Entry::You { id, held, .. } => (0, *id, *held),
                Entry::Answer { id, .. } => (1, *id, false),
                Entry::Steps { last, .. } => (2, *last, false),
                Entry::Failed { id, .. } => (3, *id, false),
                Entry::Notice { id, .. } => (4, *id, false),
                Entry::Other { id, .. } => (5, *id, false),
            };
            let retry = retry == Some(last) && kind == 3;
            let give = if kind == 4 && givable.contains_key(&last) { give } else { Give::No };
            let open = opened.contains(&entry.id());
            EntryKey { kind, first: entry.id(), last, tail: last, held, open, retry, give }
        })
        .collect()
}

pub(crate) fn arrived(old: &[EntryKey], new: &[EntryKey]) -> bool {
    old.last().map(|key| key.last) != new.last().map(|key| key.last)
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
