use serde::Deserialize;
use serde_json::{Map, Value};

use super::model::{FlowInfo, project_args};
use crate::store::args;

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct RoleDraft {
    pub base: Value,
    pub name: String,
    pub agent: String,
    pub model: String,
    pub effort: String,
    pub prompt: String,
    pub edits: bool,
    pub own_worktree: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct StageDraft {
    pub base: Value,
    pub roles: String,
    pub back_to: String,
    pub when: String,
    pub at_most: String,
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

fn number(value: &Value, key: &str) -> String {
    value.get(key).and_then(Value::as_u64).map(|n| n.to_string()).unwrap_or_default()
}

impl RoleDraft {
    pub(super) fn from_value(value: &Value) -> Self {
        Self {
            base: value.clone(),
            name: text(value, "name"),
            agent: text(value, "agent"),
            model: text(value, "model"),
            effort: text(value, "effort"),
            prompt: text(value, "prompt"),
            edits: value.get("edits").and_then(Value::as_bool).unwrap_or(false),
            own_worktree: text(value, "worktree") == "own",
        }
    }

    pub(super) fn new(name: &str, agent: &str, edits: bool) -> Self {
        Self { name: name.into(), agent: agent.into(), edits, ..Self::default() }
    }

    fn to_value(&self, lead: bool) -> Value {
        let mut map = object(&self.base);
        map.insert("name".into(), self.name.trim().into());
        map.insert("agent".into(), self.agent.trim().into());
        map.insert("model".into(), optional(&self.model));
        map.insert("effort".into(), optional(&self.effort));
        map.insert("prompt".into(), self.prompt.clone().into());
        if !lead {
            map.insert("edits".into(), self.edits.into());
            map.insert("worktree".into(), if self.own_worktree { "own" } else { "shared" }.into());
        }
        Value::Object(map)
    }
}

impl StageDraft {
    pub(super) fn from_value(value: &Value) -> Self {
        let back = value.get("back").filter(|b| b.is_object());
        Self {
            base: value.clone(),
            roles: value
                .get("roles")
                .and_then(Value::as_array)
                .map(|r| r.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", "))
                .unwrap_or_default(),
            back_to: back.map(|b| number(b, "to")).unwrap_or_default(),
            when: back.map(|b| text(b, "when")).unwrap_or_default(),
            at_most: back.map(|b| number(b, "at_most")).unwrap_or_default(),
        }
    }

    pub(super) fn with_roles(roles: &str) -> Self {
        Self { roles: roles.into(), ..Self::default() }
    }

    fn to_value(&self, number: usize) -> Result<Value, String> {
        let mut map = object(&self.base);
        let roles: Vec<Value> = role_names(&self.roles).into_iter().map(Value::from).collect();
        map.insert("roles".into(), Value::Array(roles));
        map.insert("back".into(), self.back(number)?);
        Ok(Value::Object(map))
    }

    fn back(&self, number: usize) -> Result<Value, String> {
        if self.back_to.trim().is_empty() {
            return Ok(Value::Null);
        }
        let to: u64 =
            self.back_to.trim().parse().map_err(|_| format!("stage {number}: back to takes a stage number"))?;
        let at_most: u64 =
            self.at_most.trim().parse().map_err(|_| format!("stage {number}: at most takes a number"))?;
        let mut back = object(self.base.get("back").unwrap_or(&Value::Null));
        back.insert("to".into(), to.into());
        back.insert("when".into(), self.when.trim().into());
        back.insert("at_most".into(), at_most.into());
        Ok(Value::Object(back))
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
            orchestrator: RoleDraft::new("orchestrator", "claude", false),
            roles: vec![RoleDraft::new("coder", "claude", true)],
            stages: vec![StageDraft::with_roles("coder")],
            ..Self::default()
        }
    }

    pub(super) fn to_value(&self) -> Result<Value, String> {
        let mut map = object(&self.base);
        map.insert("name".into(), self.name.trim().into());
        map.insert("description".into(), self.description.clone().into());
        map.insert("notes".into(), self.notes.clone().into());
        map.insert("end".into(), if self.commits { "commits" } else { "pull_request" }.into());
        map.insert("orchestrator".into(), self.orchestrator.to_value(true));
        map.insert("roles".into(), Value::Array(self.roles.iter().map(|r| r.to_value(false)).collect()));
        let stages: Result<Vec<Value>, String> =
            self.stages.iter().enumerate().map(|(i, s)| s.to_value(i + 1)).collect();
        map.insert("stages".into(), Value::Array(stages?));
        Ok(Value::Object(map))
    }
}

fn object(value: &Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

fn optional(text: &str) -> Value {
    let text = text.trim();
    if text.is_empty() { Value::Null } else { text.into() }
}

pub(super) fn role_names(list: &str) -> Vec<String> {
    list.split(',').map(str::trim).filter(|n| !n.is_empty()).map(str::to_string).collect()
}

pub(super) fn renamed_in(list: &str, old: &str, new: &str) -> Option<String> {
    let names = role_names(list);
    if !names.iter().any(|n| n == old) {
        return None;
    }
    let names: Vec<&str> = names.iter().map(|n| if n == old { new } else { n.as_str() }).collect();
    Some(names.join(", "))
}

pub(super) fn removed_from(list: &str, gone: &str) -> Option<String> {
    let names = role_names(list);
    let kept: Vec<&str> = names.iter().map(String::as_str).filter(|n| *n != gone).collect();
    (kept.len() != names.len()).then(|| kept.join(", "))
}

pub(super) fn fresh_role_name(taken: &[String]) -> String {
    (1..=taken.len() + 1).map(|n| format!("role {n}")).find(|name| !taken.contains(name)).unwrap_or_default()
}

#[cfg(test)]
mod tests;
