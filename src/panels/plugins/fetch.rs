use serde::Deserialize;

use super::ask::Question;
use super::data::short;
use crate::store::args;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Preview {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) version: String,
    pub(super) commit: String,
    pub(super) lines: Vec<String>,
    pub(super) replacing: Option<Replacing>,
    pub(super) up_to_date: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Replacing {
    pub(super) commit: String,
    pub(super) enabled: bool,
}

impl Preview {
    pub(super) fn parse(text: &str) -> anyhow::Result<Self> {
        Ok(serde_json::from_str(text)?)
    }

    fn label(&self) -> &str {
        if self.name.is_empty() { &self.id } else { &self.name }
    }

    pub(super) fn install_command(&self, source: &str) -> Vec<String> {
        args(&["plugin", "install", source, "--commit", &self.commit, "--yes"])
    }

    pub(super) fn update_command(&self) -> Vec<String> {
        args(&["plugin", "update", &self.id, "--commit", &self.commit, "--yes"])
    }

    pub(super) fn installed(&self) -> String {
        format!("installed {} {}, turned off", self.label(), self.version)
    }

    pub(super) fn updated(&self) -> String {
        let on = if self.replacing.as_ref().is_some_and(|r| r.enabled) { "on" } else { "off" };
        format!("updated {} to {}, still {on}", self.label(), self.version)
    }

    pub(super) fn change(&self) -> String {
        let old = self.replacing.as_ref().map_or("?", |r| short(&r.commit));
        format!("{}: {old} → {}", self.label(), short(&self.commit))
    }
}

pub(super) fn install_question(preview: &Preview) -> Question {
    Question {
        title: format!("Install {}?", preview.label()),
        message: "Nothing has run yet. Read what it runs: it is installed turned off, exactly at this commit.".into(),
        details: preview.lines.clone(),
        button: "Install",
        danger: false,
    }
}

pub(super) fn update_question(changed: &[Preview]) -> Question {
    let title = match changed {
        [one] => format!("Update {}?", one.label()),
        many => format!("Update {} plugins?", many.len()),
    };
    let details = changed
        .iter()
        .flat_map(|p| std::iter::once(p.change()).chain(p.lines.iter().map(|l| format!("  {l}"))))
        .collect();
    Question {
        title,
        message: "Each updates exactly to the commit shown, and stays on or off as it is: one that is on runs its new \
                  commands at once."
            .into(),
        details,
        button: "Update",
        danger: false,
    }
}

#[derive(Debug, Default)]
pub(super) struct Checked {
    pub(super) left: usize,
    pub(super) changed: Vec<Preview>,
    pub(super) current: Vec<String>,
    pub(super) failed: Vec<String>,
}

impl Checked {
    pub(super) fn new(left: usize) -> Self {
        Self { left, ..Self::default() }
    }

    pub(super) fn add(&mut self, result: anyhow::Result<Preview>) -> bool {
        self.left = self.left.saturating_sub(1);
        match result {
            Ok(p) if p.up_to_date => self.current.push(p.id),
            Ok(p) => self.changed.push(p),
            Err(e) => self.failed.push(e.to_string()),
        }
        self.left == 0
    }

    pub(super) fn up_to_date(&self) -> Option<String> {
        if !self.changed.is_empty() || !self.failed.is_empty() {
            return None;
        }
        match self.current.as_slice() {
            [] => None,
            [one] => Some(format!("{one} is up to date")),
            _ => Some("every installed plugin is up to date".into()),
        }
    }
}

#[cfg(test)]
mod tests;
