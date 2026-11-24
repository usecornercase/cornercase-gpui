use serde::Deserialize;

use super::model::FlowInfo;
use crate::store::args;

const FENCE: &str = "---";

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Library {
    pub dir: String,
    pub personas: Vec<Persona>,
    pub broken: Vec<Unreadable>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Persona {
    pub id: String,
    pub name: String,
    pub description: String,
    pub emoji: String,
    pub color: String,
    pub source: Option<String>,
    pub bytes: u64,
    pub path: Option<String>,
    pub replaced: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Unreadable {
    pub path: String,
    pub error: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Shown {
    pub persona: Persona,
    pub text: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Preview {
    pub personas: Vec<Persona>,
    pub check: String,
    pub saved: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Saved {
    pub persona: Persona,
}

pub(crate) fn parse_library(text: &str) -> anyhow::Result<Library> {
    Ok(serde_json::from_str(text)?)
}

pub(crate) fn parse_shown(text: &str) -> anyhow::Result<Shown> {
    Ok(serde_json::from_str(text)?)
}

pub(crate) fn parse_preview(text: &str) -> anyhow::Result<Preview> {
    Ok(serde_json::from_str(text)?)
}

pub(crate) fn parse_saved(text: &str) -> anyhow::Result<Saved> {
    Ok(serde_json::from_str(text)?)
}

impl Library {
    pub(crate) fn get(&self, id: &str) -> Option<&Persona> {
        self.personas.iter().find(|persona| persona.id == id)
    }
}

impl Persona {
    pub(crate) fn label(&self) -> String {
        if self.emoji.is_empty() { self.name.clone() } else { format!("{} {}", self.emoji, self.name) }
    }

    pub(crate) fn origin(&self) -> String {
        self.source.as_deref().map_or_else(|| "yours".to_string(), short_source)
    }
}

pub(crate) fn labeller(library: Option<&Library>) -> impl Fn(&str) -> Option<String> + use<'_> {
    move |id| library?.get(id).map(Persona::label)
}

pub(crate) fn short_source(source: &str) -> String {
    let file = source.trim_end_matches('/').rsplit(['/', '\\']).next().unwrap_or(source);
    let local = source.contains("://") || source.starts_with(['/', '.', '~']) || source.contains('\\');
    let parts: Vec<&str> = source.split('/').collect();
    match parts.as_slice() {
        [owner, repo, ..] if !local && !owner.is_empty() && !repo.is_empty() => format!("{owner}/{repo}"),
        _ => file.to_string(),
    }
}

pub(crate) fn followers(flows: &[FlowInfo], id: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for flow in flows {
        let follows = flow.flow.every_role().any(|role| role.persona.as_deref() == Some(id));
        if follows && !names.contains(&flow.name) {
            names.push(flow.name.clone());
        }
    }
    names
}

pub(crate) fn and_list(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

pub(crate) fn body(text: &str) -> &str {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let rest = front_end(text).map_or(text, |end| &text[end..]);
    rest.trim_start_matches(['\r', '\n'])
}

fn front_end(text: &str) -> Option<usize> {
    let mut lines = text.split_inclusive('\n');
    if lines.next()?.trim_end() != FENCE {
        return None;
    }
    let mut at = text.find('\n')? + 1;
    for line in lines {
        at += line.len();
        if line.trim_end() == FENCE {
            return Some(at);
        }
    }
    None
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Draft {
    pub name: String,
    pub description: String,
    pub emoji: String,
    pub body: String,
}

impl Draft {
    pub(crate) fn of(shown: &Shown) -> Self {
        Self {
            name: shown.persona.name.clone(),
            description: shown.persona.description.clone(),
            emoji: shown.persona.emoji.clone(),
            body: body(&shown.text).to_string(),
        }
    }

    pub(crate) fn problem(&self) -> Option<&'static str> {
        if self.name.trim().is_empty() {
            Some("Give the persona a name.")
        } else if self.body.trim().is_empty() {
            Some("Write what the agent should do and how.")
        } else {
            None
        }
    }

    pub(crate) fn args(&self, editing: Option<&str>) -> Vec<String> {
        let mut command = match editing {
            Some(id) => args(&["persona", "edit", id, "--instructions=-"]),
            None => args(&["persona", "new", "-"]),
        };
        command.push(format!("--name={}", self.name.trim()));
        let editing = editing.is_some();
        for (flag, value) in [("description", &self.description), ("emoji", &self.emoji)] {
            if editing || !value.trim().is_empty() {
                command.push(format!("--{flag}={}", value.trim()));
            }
        }
        command.push("--json".into());
        command
    }
}

pub(crate) fn import_args(source: &str, check: bool, replace: bool) -> Vec<String> {
    let mut command = args(&["persona", "import", if check { "--check" } else { "--yes" }, "--json"]);
    if replace || check {
        command.push("--replace".into());
    }
    command.extend(["--".into(), source.trim().to_string()]);
    command
}

pub(crate) fn shown_args(id: &str) -> Vec<String> {
    args(&["persona", "show", id, "--json"])
}

pub(crate) fn remove_args(id: &str) -> Vec<String> {
    args(&["persona", "remove", id, "--json"])
}

pub(crate) const LIST: [&str; 3] = ["persona", "list", "--json"];

impl Preview {
    pub(crate) fn replaces(&self) -> bool {
        self.personas.iter().any(|persona| persona.replaced)
    }

    pub(crate) fn matching(&self, filter: &str) -> Vec<&Persona> {
        let filter = filter.trim().to_lowercase();
        let hit = |persona: &&Persona| {
            [&persona.id, &persona.name, &persona.description].iter().any(|text| text.to_lowercase().contains(&filter))
        };
        self.personas.iter().filter(hit).collect()
    }
}

pub(crate) fn count(count: usize) -> String {
    if count == 1 { "1 persona".into() } else { format!("{count} personas") }
}

pub(crate) fn kib(bytes: u64) -> String {
    format!("{} KiB", bytes.div_ceil(1024))
}

#[cfg(test)]
mod tests;
