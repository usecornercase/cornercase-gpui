use std::ops::Range;

use crate::model::Shown;

const FUZZY: u8 = 4;
const CONTEXT: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Kind {
    Group,
    Project,
    Workspace,
    Agent,
    Tab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Goto {
    Group { id: u64, first: Option<u64> },
    Project(u64),
    Workspace(u64),
    Tab(u64),
    Agent { pane: u64, tab: u64 },
}

impl Goto {
    pub(super) fn focus(self) -> Option<(&'static str, u64)> {
        match self {
            Self::Group { first, .. } => first.map(|id| ("project", id)),
            Self::Project(id) => Some(("project", id)),
            Self::Workspace(id) => Some(("workspace", id)),
            Self::Tab(id) => Some(("tab", id)),
            Self::Agent { pane, .. } => Some(("pane", pane)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Candidate {
    pub(super) kind: Kind,
    pub(super) goto: Goto,
    pub(super) name: String,
    pub(super) detail: String,
    pub(super) place: String,
    pub(super) keys: Vec<String>,
    pub(super) status: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Marks {
    pub(super) name: Vec<Range<usize>>,
    pub(super) detail: Vec<Range<usize>>,
    pub(super) place: Vec<Range<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Ranked {
    pub(super) candidate: Candidate,
    pub(super) marks: Marks,
}

fn best_key(candidate: &Candidate, query: &str) -> Option<(u8, usize, Vec<Range<usize>>)> {
    candidate
        .keys
        .iter()
        .enumerate()
        .filter_map(|(i, key)| key_rank(key, query).filter(|(rank, _)| i == 0 || *rank < FUZZY).map(|(r, s)| (r, i, s)))
        .min_by_key(|(rank, i, _)| (*rank, *i))
}

fn marks(candidate: &Candidate, key: usize, ranges: Vec<Range<usize>>, query: &str) -> Marks {
    let name_key = candidate.keys.first().map_or("", String::as_str);
    if key == 0 {
        if !candidate.name.ends_with(name_key) {
            return Marks::default();
        }
        let shift = candidate.name.len() - name_key.len();
        return Marks {
            name: ranges.into_iter().map(|r| r.start + shift..r.end + shift).collect(),
            ..Marks::default()
        };
    }
    match find(&candidate.detail, query) {
        Some(range) => Marks { detail: vec![range], ..Marks::default() },
        None => Marks { place: find(&candidate.place, query).into_iter().collect(), ..Marks::default() },
    }
}

fn rank_one(order: usize, candidate: Candidate, query: &str) -> Option<(u8, usize, Ranked)> {
    let (rank, key, ranges) = best_key(&candidate, query)?;
    let marks = marks(&candidate, key, ranges, query);
    Some((rank, order, Ranked { candidate, marks }))
}

pub(super) fn rank(candidates: Vec<Candidate>, query: &str) -> Vec<Ranked> {
    let query = query.trim();
    if query.is_empty() {
        return candidates.into_iter().map(|candidate| Ranked { candidate, marks: Marks::default() }).collect();
    }
    let mut hits: Vec<(u8, usize, Ranked)> =
        candidates.into_iter().enumerate().filter_map(|(order, candidate)| rank_one(order, candidate, query)).collect();
    hits.sort_by_key(|(rank, order, r)| (*rank, r.candidate.kind, *order));
    hits.into_iter().map(|(_, _, r)| r).collect()
}

pub(super) fn shown_row(ranked: &[Ranked], shown: Shown) -> Option<usize> {
    let at = |wanted: &dyn Fn(Goto) -> bool| ranked.iter().position(|r| wanted(r.candidate.goto));
    let agent = |pane: u64| at(&|goto| matches!(goto, Goto::Agent { pane: p, .. } if p == pane));
    let tab = |tab: u64| at(&|goto| matches!(goto, Goto::Tab(t) | Goto::Agent { tab: t, .. } if t == tab));
    shown
        .pane
        .and_then(agent)
        .or_else(|| shown.tab.and_then(tab))
        .or_else(|| shown.workspace.and_then(|id| at(&|goto| goto == Goto::Workspace(id))))
        .or_else(|| shown.project.and_then(|id| at(&|goto| goto == Goto::Project(id))))
}

pub(super) fn context_top(ranked: &[Ranked], selected: usize) -> usize {
    let floor = selected.saturating_sub(CONTEXT);
    (floor..=selected)
        .rev()
        .find(|&i| ranked.get(i).is_some_and(|r| r.candidate.kind == Kind::Project))
        .unwrap_or(floor)
}

pub(super) fn listing_status(query: &str, shown: usize, total: usize) -> String {
    let query = query.trim();
    match (query.is_empty(), total) {
        (true, 0) => "No project is open: open one with ⌘O".into(),
        (true, n) if n > shown => format!("The first {shown} of {n} in the sidebar's order · type to find the rest"),
        (true, _) => {
            "Everything open, in the sidebar's order · type to find it by name, branch, folder, agent or model".into()
        }
        (false, 0) => format!("Nothing matches “{query}”"),
        (false, n) if n > shown => format!("The first {shown} of {n} results"),
        (false, n) => format!("{n} result{}", if n == 1 { "" } else { "s" }),
    }
}

fn letters(text: &str) -> Vec<char> {
    fold(text).into_iter().map(|(_, c)| c).collect()
}

pub(super) fn key_rank(key: &str, query: &str) -> Option<(u8, Vec<Range<usize>>)> {
    let hay = fold(key);
    let needle = letters(query);
    if needle.is_empty() || needle.len() > hay.len() {
        return None;
    }
    let chars: Vec<char> = hay.iter().map(|(_, c)| *c).collect();
    let span = |start: usize| vec![Range { start: hay[start].0.start, end: hay[start + needle.len() - 1].0.end }];
    let starts: Vec<usize> =
        (0..=chars.len() - needle.len()).filter(|&i| chars[i..i + needle.len()] == needle[..]).collect();
    let Some(&first) = starts.first() else { return in_order(&hay, &needle) };
    if first == 0 {
        return Some((u8::from(chars.len() != needle.len()), span(first)));
    }
    match starts.iter().find(|&&i| !chars[i - 1].is_alphanumeric()) {
        Some(&word) => Some((2, span(word))),
        None => Some((3, span(first))),
    }
}

fn in_order(hay: &[(Range<usize>, char)], needle: &[char]) -> Option<(u8, Vec<Range<usize>>)> {
    let mut ranges = Vec::new();
    let mut next = 0;
    for (range, c) in hay {
        if next < needle.len() && *c == needle[next] {
            ranges.push(range.clone());
            next += 1;
        }
    }
    (next == needle.len()).then_some((FUZZY, ranges))
}

fn fold(text: &str) -> Vec<(Range<usize>, char)> {
    text.char_indices().map(|(i, c)| (i..i + c.len_utf8(), c.to_lowercase().next().unwrap_or(c))).collect()
}

pub(super) fn find(text: &str, query: &str) -> Option<Range<usize>> {
    let hay = fold(text);
    let needle = letters(query.trim());
    if needle.is_empty() || needle.len() > hay.len() {
        return None;
    }
    (0..=hay.len() - needle.len())
        .find(|&i| hay[i..i + needle.len()].iter().map(|(_, c)| *c).eq(needle.iter().copied()))
        .map(|i| hay[i].0.start..hay[i + needle.len() - 1].0.end)
}
