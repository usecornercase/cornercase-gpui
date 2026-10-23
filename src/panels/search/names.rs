use std::ops::Range;

use crate::model::{Group, Project, Session, Workspace, status_rank};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Kind {
    Group,
    Project,
    Workspace,
    Tab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Goto {
    Group { id: u64, first: Option<u64> },
    Project(u64),
    Workspace(u64),
    Tab(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Candidate {
    pub(super) kind: Kind,
    pub(super) goto: Goto,
    pub(super) name: String,
    pub(super) context: String,
    pub(super) keys: Vec<String>,
    pub(super) status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Ranked {
    pub(super) candidate: Candidate,
    pub(super) highlight: Vec<Range<usize>>,
}

type Grouped<'a> = Vec<(Option<u64>, Vec<&'a Project>)>;

fn grouped(session: &Session) -> Grouped<'_> {
    let mut ordered: Grouped = Vec::new();
    for project in &session.projects {
        let group = project.group.filter(|g| session.groups.iter().any(|x| x.id == *g));
        match ordered.iter_mut().find(|(g, _)| *g == group) {
            Some((_, list)) => list.push(project),
            None => ordered.push((group, vec![project])),
        }
    }
    ordered
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

fn group_candidate(group: &Group, ordered: &Grouped) -> Candidate {
    let projects = ordered.iter().find(|(id, _)| *id == Some(group.id)).map_or(&[][..], |(_, ps)| ps.as_slice());
    Candidate {
        kind: Kind::Group,
        goto: Goto::Group { id: group.id, first: projects.first().map(|p| p.id) },
        name: group.name.clone(),
        context: format!("{} project{}", projects.len(), plural(projects.len())),
        keys: vec![group.name.clone()],
        status: projects.iter().filter_map(|p| p.loudest()).max_by_key(|s| status_rank(Some(s))).map(String::from),
    }
}

fn project_candidates(project: &Project, group_name: Option<&str>, out: &mut Vec<Candidate>) {
    out.push(Candidate {
        kind: Kind::Project,
        goto: Goto::Project(project.id),
        name: project.name.clone(),
        context: group_name.unwrap_or_default().to_string(),
        keys: vec![project.name.clone()],
        status: project.loudest().map(String::from),
    });
    for workspace in &project.workspaces {
        workspace_candidates(project, workspace, out);
    }
}

fn workspace_candidates(project: &Project, workspace: &Workspace, out: &mut Vec<Candidate>) {
    let mut keys = vec![workspace.name.clone()];
    keys.extend(workspace.branch.clone().filter(|b| *b != workspace.name));
    out.push(Candidate {
        kind: Kind::Workspace,
        goto: Goto::Workspace(workspace.id),
        name: if workspace.worktree { format!("⎇ {}", workspace.name) } else { workspace.name.clone() },
        context: project.name.clone(),
        keys: keys.clone(),
        status: workspace.loudest().map(String::from),
    });
    out.extend(workspace.tabs.iter().map(|tab| Candidate {
        kind: Kind::Tab,
        goto: Goto::Tab(tab.id),
        name: tab.name.clone(),
        context: format!("{} › {}", project.name, workspace.name),
        keys: std::iter::once(tab.name.clone()).chain(keys.iter().cloned()).collect(),
        status: tab.loudest().map(String::from),
    }));
}

pub(super) fn candidates(session: &Session) -> Vec<Candidate> {
    let ordered = grouped(session);
    let mut out: Vec<Candidate> = session.groups.iter().map(|g| group_candidate(g, &ordered)).collect();
    for (group, projects) in &ordered {
        let group_name = group.and_then(|g| session.groups.iter().find(|x| x.id == g)).map(|g| g.name.as_str());
        for project in projects {
            project_candidates(project, group_name, &mut out);
        }
    }
    out
}

fn rank_one(order: usize, candidate: Candidate, query: &str) -> Option<(u8, usize, Ranked)> {
    let ((rank, ranges), key) = candidate
        .keys
        .iter()
        .enumerate()
        .filter_map(|(i, key)| key_rank(key, query).map(|r| (r, i)))
        .min_by_key(|((rank, _), i)| (*rank, *i))?;
    let shift = candidate.name.len() - candidate.keys[0].len().min(candidate.name.len());
    let highlight = if key == 0 && candidate.name.ends_with(candidate.keys[0].as_str()) {
        ranges.into_iter().map(|r| r.start + shift..r.end + shift).collect()
    } else {
        Vec::new()
    };
    Some((rank, order, Ranked { candidate, highlight }))
}

pub(super) fn rank(candidates: Vec<Candidate>, query: &str) -> Vec<Ranked> {
    let query = query.trim();
    if query.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<(u8, usize, Ranked)> =
        candidates.into_iter().enumerate().filter_map(|(order, candidate)| rank_one(order, candidate, query)).collect();
    hits.sort_by_key(|(rank, order, r)| (*rank, r.candidate.kind, *order));
    hits.into_iter().map(|(_, _, r)| r).collect()
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
    (next == needle.len()).then_some((4, ranges))
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
