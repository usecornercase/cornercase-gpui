use std::cmp::Ordering;
use std::ops::RangeInclusive;

use serde::Deserialize;
use serde_json::{Map, Value};

use super::model::{FlowInfo, FlowSpec, RoleSpec, project_args};
use crate::store::args;

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct RoleDraft {
    pub base: Value,
    pub name: String,
    pub agent: String,
    pub model: String,
    pub effort: String,
    pub args: String,
    pub env: String,
    pub prompt: String,
    pub edits: bool,
    pub own_worktree: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct StageDraft {
    pub base: Value,
    pub roles: Vec<String>,
    pub back: Option<BackDraft>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct BackDraft {
    pub to: usize,
    pub when: String,
    pub at_most: u32,
}

impl Default for BackDraft {
    fn default() -> Self {
        Self { to: 1, when: "problems".into(), at_most: 3 }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Draft {
    pub base: Value,
    pub name: String,
    pub description: String,
    pub notes: String,
    pub commits: bool,
    pub orchestrator: RoleDraft,
    pub roles: Vec<RoleDraft>,
    pub stages: Vec<StageDraft>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Place {
    File(String),
    Yours,
    Project,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(super) struct Saved {
    pub name: String,
    pub origin: String,
    pub path: Option<String>,
    pub problems: Vec<String>,
    pub check: Option<String>,
    #[serde(rename = "saved")]
    pub written: bool,
}

pub(super) fn parse_saved(text: &str) -> anyhow::Result<Saved> {
    Ok(serde_json::from_str(text)?)
}

pub(super) fn save_args(place: &Place, project: Option<u64>, check: bool) -> Vec<String> {
    let mut command = args(&["flow", "save", "-", "--json"]);
    match place {
        Place::File(path) => command.extend(["--path".into(), path.clone()]),
        Place::Project => command.extend(["--to".into(), "project".into()]),
        Place::Yours => {}
    }
    command.extend(project_args(project));
    if check {
        command.push("--check".into());
    }
    command
}

pub(super) fn place_of(flow: &FlowInfo) -> Place {
    match &flow.path {
        Some(path) => Place::File(path.clone()),
        None => Place::Yours,
    }
}

pub(super) fn has_breaks(text: &str) -> bool {
    text.contains('\n')
}

fn text(value: &Value, key: &str) -> String {
    value.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn arguments(value: &Value) -> String {
    let words = value.get("args").and_then(Value::as_array).into_iter().flatten();
    join_words(words.filter_map(Value::as_str).map(str::to_string))
}

fn variables(value: &Value) -> String {
    let pairs = value.get("env").and_then(Value::as_object).into_iter().flatten();
    join_words(pairs.map(|(key, value)| format!("{key}={}", value.as_str().unwrap_or_default())))
}

impl RoleDraft {
    pub(super) fn from_value(value: &Value) -> Self {
        Self {
            base: value.clone(),
            name: text(value, "name"),
            agent: text(value, "agent"),
            model: text(value, "model"),
            effort: text(value, "effort"),
            args: arguments(value),
            env: variables(value),
            prompt: text(value, "prompt"),
            edits: value.get("edits").and_then(Value::as_bool).unwrap_or(false),
            own_worktree: text(value, "worktree") == "own",
        }
    }

    pub(super) fn new(name: &str, agent: &str, edits: bool) -> Self {
        Self { name: name.into(), agent: agent.into(), edits, ..Self::default() }
    }

    fn to_value(&self, lead: bool) -> Result<Value, String> {
        let mut map = object(&self.base);
        map.insert("agent".into(), self.agent.trim().into());
        map.insert("model".into(), optional(&self.model));
        map.insert("effort".into(), optional(&self.effort));
        map.insert("args".into(), split_words(&self.args).into());
        map.insert("env".into(), self.env_value(lead)?);
        map.insert("prompt".into(), self.prompt.clone().into());
        if !lead {
            map.insert("name".into(), self.name.trim().into());
            map.insert("edits".into(), self.edits.into());
            map.insert("worktree".into(), if self.own_worktree { "own" } else { "shared" }.into());
        }
        Ok(Value::Object(map))
    }

    fn env_value(&self, lead: bool) -> Result<Value, String> {
        let mut env = Map::new();
        let who = if lead { "orchestrator" } else { self.name.trim() };
        for pair in split_words(&self.env) {
            let Some((key, value)) = pair_of(&pair) else {
                return Err(format!("{who}: variables are KEY=value, and `{pair}` has no ="));
            };
            env.insert(key.to_string(), value.into());
        }
        Ok(Value::Object(env))
    }

    pub(super) fn spec(&self) -> RoleSpec {
        let pairs = split_words(&self.env)
            .into_iter()
            .filter_map(|pair| pair_of(&pair).map(|(key, value)| (key.to_string(), Value::from(value))));
        let model = self.model.trim();
        let effort = self.effort.trim();
        RoleSpec {
            name: self.name.trim().into(),
            agent: self.agent.trim().into(),
            model: (!model.is_empty()).then(|| model.into()),
            effort: (!effort.is_empty()).then(|| effort.into()),
            args: split_words(&self.args),
            env: pairs.collect(),
            edits: self.edits,
            worktree: if self.own_worktree { "own" } else { "shared" }.into(),
            prompt: self.prompt.clone(),
        }
    }
}

fn pair_of(pair: &str) -> Option<(&str, &str)> {
    pair.split_once('=').filter(|(key, _)| !key.is_empty())
}

impl StageDraft {
    pub(super) fn from_value(value: &Value) -> Self {
        let defaults = BackDraft::default();
        let back = value.get("back").filter(|b| b.is_object()).map(|b| BackDraft {
            to: b.get("to").and_then(Value::as_u64).map_or(defaults.to, |n| usize::try_from(n).unwrap_or(usize::MAX)),
            when: b.get("when").and_then(Value::as_str).map_or(defaults.when, str::to_string),
            at_most: b
                .get("at_most")
                .and_then(Value::as_u64)
                .map_or(defaults.at_most, |n| u32::try_from(n).unwrap_or(u32::MAX)),
        });
        let roles = value.get("roles").and_then(Value::as_array).into_iter().flatten();
        Self { base: value.clone(), roles: roles.filter_map(Value::as_str).map(str::to_string).collect(), back }
    }

    fn to_value(&self) -> Value {
        let mut map = object(&self.base);
        map.insert("roles".into(), self.roles.iter().cloned().map(Value::from).collect());
        let back = self.back.as_ref().map_or(Value::Null, |back| back.to_value(&self.base));
        map.insert("back".into(), back);
        Value::Object(map)
    }
}

impl BackDraft {
    fn to_value(&self, base: &Value) -> Value {
        let mut back = object(base.get("back").unwrap_or(&Value::Null));
        back.insert("to".into(), self.to.into());
        back.insert("when".into(), self.when.trim().into());
        back.insert("at_most".into(), self.at_most.into());
        Value::Object(back)
    }
}

impl Draft {
    pub(super) fn from_value(value: &Value) -> Self {
        let list = |key: &str| value.get(key).and_then(Value::as_array).cloned().unwrap_or_default();
        Self {
            base: value.clone(),
            name: text(value, "name"),
            description: text(value, "description"),
            notes: text(value, "notes"),
            commits: text(value, "end") == "commits",
            orchestrator: RoleDraft::from_value(value.get("orchestrator").unwrap_or(&Value::Null)),
            roles: list("roles").iter().map(RoleDraft::from_value).collect(),
            stages: list("stages").iter().map(StageDraft::from_value).collect(),
        }
    }

    pub(super) fn blank(name: &str) -> Self {
        Self {
            name: name.into(),
            commits: true,
            orchestrator: RoleDraft {
                base: serde_json::json!({"name": "orchestrator"}),
                ..RoleDraft::new("orchestrator", "claude", false)
            },
            roles: vec![RoleDraft::new("coder", "claude", true)],
            stages: vec![StageDraft { roles: vec!["coder".into()], ..StageDraft::default() }],
            ..Self::default()
        }
    }

    pub(super) fn to_value(&self) -> Result<Value, String> {
        let mut map = object(&self.base);
        map.insert("name".into(), self.name.trim().into());
        map.insert("description".into(), self.description.clone().into());
        map.insert("notes".into(), self.notes.clone().into());
        map.insert("end".into(), if self.commits { "commits" } else { "pull_request" }.into());
        map.insert("orchestrator".into(), self.orchestrator.to_value(true)?);
        let roles: Result<Vec<Value>, String> = self.roles.iter().map(|r| r.to_value(false)).collect();
        map.insert("roles".into(), Value::Array(roles?));
        map.insert("stages".into(), self.stages.iter().map(StageDraft::to_value).collect());
        Ok(Value::Object(map))
    }

    pub(super) fn plan(&self) -> Option<FlowSpec> {
        serde_json::from_value(self.to_value().ok()?).ok()
    }
}

fn object(value: &Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

fn optional(text: &str) -> Value {
    let text = text.trim();
    if text.is_empty() { Value::Null } else { text.into() }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum LoopAfter {
    Same,
    To(usize),
    Gone,
}

pub(super) fn loop_after(to: usize, number: usize, moved: impl Fn(usize) -> Option<usize>) -> LoopAfter {
    match moved(to) {
        Some(target) if target == 0 || target >= number => LoopAfter::Gone,
        Some(target) if target == to => LoopAfter::Same,
        Some(target) => LoopAfter::To(target),
        None => LoopAfter::Gone,
    }
}

pub(super) fn swapped(first: usize, second: usize) -> impl Fn(usize) -> Option<usize> {
    move |stage| {
        Some(if stage == first {
            second
        } else if stage == second {
            first
        } else {
            stage
        })
    }
}

pub(super) fn without(gone: usize) -> impl Fn(usize) -> Option<usize> {
    move |stage| match stage.cmp(&gone) {
        Ordering::Less => Some(stage),
        Ordering::Equal => None,
        Ordering::Greater => Some(stage - 1),
    }
}

pub(super) fn neighbour(index: usize, up: bool, len: usize) -> Option<usize> {
    let other = if up { index.checked_sub(1) } else { Some(index + 1) };
    other.filter(|other| *other < len && index < len)
}

fn numbers(stages: &[usize]) -> String {
    let Some((last, rest)) = stages.split_last() else { return String::new() };
    if rest.is_empty() {
        return last.to_string();
    }
    let names: Vec<String> = rest.iter().map(ToString::to_string).collect();
    format!("{} and {last}", names.join(", "))
}

pub(super) fn gone_note(stages: &[usize]) -> Option<String> {
    let (_, rest) = stages.split_last()?;
    Some(if rest.is_empty() {
        format!("Stage {} lost its loop: it no longer goes back to an earlier stage", numbers(stages))
    } else {
        format!("Stages {} lost their loops: they no longer go back to an earlier stage", numbers(stages))
    })
}

pub(super) fn split_words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word: Option<String> = None;
    let mut quote = None;
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(open), c) if c == open => quote = None,
            (Some('"') | None, '\\') => word.get_or_insert_default().extend(chars.next()),
            (None, c) if c.is_whitespace() => words.extend(word.take()),
            (None, '"' | '\'') => {
                quote = Some(c);
                word.get_or_insert_default();
            }
            (_, c) => word.get_or_insert_default().push(c),
        }
    }
    words.extend(word);
    words
}

fn join_words(words: impl Iterator<Item = String>) -> String {
    words.map(|word| quoted(&word)).collect::<Vec<_>>().join(" ")
}

fn quoted(word: &str) -> String {
    let plain = |c: char| c.is_ascii_alphanumeric() || "-_./:@%+=,".contains(c);
    if !word.is_empty() && word.chars().all(plain) {
        return word.to_string();
    }
    format!("'{}'", word.replace('\'', r"'\''"))
}

pub(super) fn fresh_role_name(taken: &[String]) -> String {
    (1..=taken.len() + 1).map(|n| format!("role {n}")).find(|name| !taken.contains(name)).unwrap_or_default()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Back {
    pub to: usize,
    pub at_most: u32,
}

impl Back {
    pub(super) fn step(self, up: bool) -> Self {
        let at_most = if up { self.at_most.saturating_add(1) } else { self.at_most.saturating_sub(1).max(1) };
        Self { at_most, ..self }
    }

    pub(super) fn lowest(self) -> bool {
        self.at_most <= 1
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Stage<W> {
    pub base: Value,
    pub roles: Vec<u64>,
    pub back: Option<Back>,
    pub when: W,
}

impl<W> Stage<W> {
    pub(super) fn of(role: u64, when: W) -> Self {
        Self { base: Value::Null, roles: vec![role], back: None, when }
    }

    pub(super) fn with_when<V>(self, make: impl FnOnce(W) -> V) -> Stage<V> {
        Stage { base: self.base, roles: self.roles, back: self.back, when: make(self.when) }
    }

    pub(super) fn draft(&self, names: &[(u64, String)], when: String) -> StageDraft {
        let picked = self.roles.iter().filter_map(|id| names.iter().find(|(role, _)| role == id));
        StageDraft {
            base: self.base.clone(),
            roles: picked.map(|(_, name)| name.clone()).collect(),
            back: self.back.map(|back| BackDraft { to: back.to, when, at_most: back.at_most }),
        }
    }
}

pub(super) fn targets(index: usize) -> RangeInclusive<usize> {
    1..=index
}

pub(super) fn stage_of<W>(stages: &[Stage<W>], role: u64) -> Option<usize> {
    stages.iter().position(|stage| stage.roles.contains(&role))
}

pub(super) fn alone<W>(stages: &[Stage<W>], role: u64) -> bool {
    stage_of(stages, role).is_some_and(|index| stages[index].roles == [role])
}

pub(super) fn move_role<W>(stages: &mut Vec<Stage<W>>, role: u64, to: usize) -> Vec<usize> {
    let Some(from) = stage_of(stages, role).filter(|from| *from != to && to < stages.len()) else {
        return Vec::new();
    };
    stages[to].roles.push(role);
    release(stages, from, role)
}

pub(super) fn remove_role<W>(stages: &mut Vec<Stage<W>>, role: u64) -> Vec<usize> {
    match stage_of(stages, role) {
        Some(from) => release(stages, from, role),
        None => Vec::new(),
    }
}

fn release<W>(stages: &mut Vec<Stage<W>>, from: usize, role: u64) -> Vec<usize> {
    stages[from].roles.retain(|id| *id != role);
    if !stages[from].roles.is_empty() {
        return Vec::new();
    }
    stages.remove(from);
    follow_loops(stages, without(from + 1))
}

pub(super) fn follow_loops<W>(stages: &mut [Stage<W>], moved: impl Fn(usize) -> Option<usize>) -> Vec<usize> {
    let mut gone = Vec::new();
    for (index, stage) in stages.iter_mut().enumerate() {
        let Some(back) = stage.back else { continue };
        match loop_after(back.to, index + 1, &moved) {
            LoopAfter::Same => {}
            LoopAfter::To(to) => stage.back = Some(Back { to, ..back }),
            LoopAfter::Gone => {
                stage.back = None;
                gone.push(index + 1);
            }
        }
    }
    gone
}

pub(super) struct Seating {
    pub stages: Vec<Stage<String>>,
    pub changes: Vec<String>,
}

impl Seating {
    pub(super) fn note(&self) -> Option<String> {
        (!self.changes.is_empty()).then(|| format!("Opened with changes: {}", self.changes.join("; ")))
    }
}

struct Seater<'a> {
    names: &'a [String],
    first: Vec<Option<usize>>,
    changes: Vec<String>,
}

impl Seater<'_> {
    fn label(&self, id: usize) -> String {
        match self.names.get(id) {
            Some(name) if !name.is_empty() => name.clone(),
            _ => format!("role {}", id + 1),
        }
    }

    fn stage(&mut self, number: usize, stage: &StageDraft) -> Stage<String> {
        let roles = stage.roles.iter().filter_map(|name| self.name(number, name)).collect();
        let back = stage.back.as_ref().map(|back| Back { to: back.to, at_most: back.at_most });
        let when = stage.back.as_ref().map(|back| back.when.clone()).unwrap_or_default();
        Stage { base: stage.base.clone(), roles, back, when }
    }

    fn name(&mut self, number: usize, name: &str) -> Option<u64> {
        let free = self.names.iter().enumerate().position(|(id, n)| n == name && self.first[id].is_none());
        let Some(id) = free else {
            self.refuse(number, name);
            return None;
        };
        self.first[id] = Some(number);
        Some(id as u64)
    }

    fn refuse(&mut self, number: usize, name: &str) {
        let first = self.names.iter().position(|n| n == name).and_then(|id| self.first[id]);
        self.changes.push(match first {
            None => format!("stage {number} named {name}, which is not a role, so it is left out"),
            Some(first) if first == number => format!("stage {number} named {name} twice, so it is there once"),
            Some(first) => format!("{name} was in stage {first} and stage {number}, so it stays in stage {first}"),
        });
    }

    fn drop_empty(&mut self, stages: &mut Vec<Stage<String>>) {
        let mut files: Vec<usize> = (1..=stages.len()).collect();
        let empty: Vec<usize> = (0..stages.len()).filter(|index| stages[*index].roles.is_empty()).rev().collect();
        for index in empty {
            let number = files.remove(index);
            stages.remove(index);
            let lost: Vec<usize> = follow_loops(stages, without(index + 1)).into_iter().map(|n| files[n - 1]).collect();
            self.changes.push(gone_stage(number, &lost));
        }
    }

    fn seat_the_rest(&mut self, stages: &mut Vec<Stage<String>>) {
        for id in (0..self.names.len()).filter(|id| self.first[*id].is_none()) {
            stages.push(Stage::of(id as u64, String::new()));
            self.changes.push(format!("{} had no stage, so it has one of its own", self.label(id)));
        }
    }
}

fn gone_stage(number: usize, lost: &[usize]) -> String {
    let base = format!("stage {number} had no roles, so it is gone");
    match lost {
        [] => base,
        [one] => format!("{base}, and so is the loop of stage {one}"),
        many => format!("{base}, and so are the loops of stages {}", numbers(many)),
    }
}

pub(super) fn seat(draft: &Draft) -> Seating {
    let names: Vec<String> = draft.roles.iter().map(|role| role.name.clone()).collect();
    let mut seater = Seater { names: &names, first: vec![None; names.len()], changes: Vec::new() };
    let mut stages: Vec<Stage<String>> =
        draft.stages.iter().enumerate().map(|(index, stage)| seater.stage(index + 1, stage)).collect();
    seater.drop_empty(&mut stages);
    seater.seat_the_rest(&mut stages);
    Seating { stages, changes: seater.changes }
}

pub(super) fn reports(roles: usize) -> &'static str {
    match roles {
        0 | 1 => "If it reports",
        2 => "If either reports",
        _ => "If any of them reports",
    }
}

pub(super) fn first_line(text: &str) -> Option<&str> {
    text.lines().map(str::trim).find(|line| !line.is_empty())
}

pub(super) fn saves_line(place: &Place, note: Option<&str>) -> String {
    let base = match place {
        Place::File(path) => return format!("Saves {path}"),
        Place::Yours => "Saves into your flows".to_string(),
        Place::Project => "Saves into this project's flows".to_string(),
    };
    match note {
        Some(note) => format!("{base} · {note}"),
        None => base,
    }
}

pub(super) fn place_note(place: &Place) -> &'static str {
    match place {
        Place::Project => "Kept in the project's .cornercase/flows, for whoever opens it.",
        Place::Yours | Place::File(_) => "Every project of yours can start it.",
    }
}

#[cfg(test)]
mod tests;
