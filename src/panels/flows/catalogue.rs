use std::collections::BTreeSet;

use serde::Deserialize;

use super::library::{Persona, and_list, count};
use crate::store::args;

const UNSORTED: &str = "Not in a division";

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Catalogue {
    pub label: String,
    pub page: Option<String>,
    pub web: bool,
    pub check: String,
    pub skipped: usize,
    pub divisions: Vec<Division>,
    pub personas: Vec<Offered>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Division {
    pub key: String,
    pub label: Option<String>,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Offered {
    pub file: String,
    pub division: Option<String>,
    pub problem: Option<String>,
    #[serde(flatten)]
    pub persona: Persona,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Group<'a> {
    pub key: Option<&'a str>,
    pub label: String,
    pub offered: Vec<&'a Offered>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ticked {
    None,
    Some,
    All,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Chosen(BTreeSet<String>);

pub(crate) fn parse_catalogue(text: &str) -> anyhow::Result<Catalogue> {
    Ok(serde_json::from_str(text)?)
}

impl Offered {
    pub(crate) fn importable(&self) -> bool {
        self.problem.is_none() && !self.persona.id.is_empty()
    }

    pub(crate) fn title(&self) -> String {
        if self.persona.name.is_empty() { self.file.clone() } else { self.persona.label() }
    }

    fn matches(&self, filter: &str) -> bool {
        let persona = &self.persona;
        [&persona.id, &persona.name, &persona.description, &self.file]
            .iter()
            .any(|text| text.to_lowercase().contains(filter))
    }
}

impl Catalogue {
    pub(crate) fn groups(&self, filter: &str) -> Vec<Group<'_>> {
        let filter = filter.trim().to_lowercase();
        let mut keys: Vec<Option<&str>> = self.divisions.iter().map(|division| Some(division.key.as_str())).collect();
        for offered in &self.personas {
            let key = offered.division.as_deref();
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
        keys.sort_by_key(Option::is_none);
        keys.into_iter()
            .map(|key| Group { key, label: self.label_of(key), offered: self.in_division(key, &filter) })
            .filter(|group| !group.offered.is_empty())
            .collect()
    }

    fn in_division(&self, key: Option<&str>, filter: &str) -> Vec<&Offered> {
        self.personas.iter().filter(|offered| offered.division.as_deref() == key && offered.matches(filter)).collect()
    }

    fn label_of(&self, key: Option<&str>) -> String {
        let Some(key) = key else { return UNSORTED.into() };
        let division = self.divisions.iter().find(|division| division.key == key);
        division.and_then(|division| division.label.clone()).unwrap_or_else(|| key.to_string())
    }

    pub(crate) fn importable(&self) -> usize {
        self.personas.iter().filter(|offered| offered.importable()).count()
    }

    pub(crate) fn summary(&self) -> String {
        let problems = self.personas.len() - self.importable();
        let mut parts = vec![count(self.importable()) + " to pick from"];
        if problems > 0 {
            parts.push(format!("{problems} that cannot be imported"));
        }
        if self.skipped > 0 {
            parts.push(format!("{} skipped as not personas", files(self.skipped)));
        }
        format!("{}.", and_list(&parts))
    }
}

pub(crate) fn sentence(text: &str) -> String {
    let mut chars = text.trim().chars();
    let Some(first) = chars.next() else { return String::new() };
    let rest = chars.as_str().trim_end_matches('.');
    format!("{}{rest}.", first.to_uppercase())
}

fn files(n: usize) -> String {
    if n == 1 { "1 file".into() } else { format!("{n} files") }
}

impl Chosen {
    pub(crate) fn has(&self, offered: &Offered) -> bool {
        self.0.contains(&offered.persona.id)
    }

    pub(crate) fn toggle(&mut self, offered: &Offered) {
        if !offered.importable() {
            return;
        }
        if !self.0.remove(&offered.persona.id) {
            self.0.insert(offered.persona.id.clone());
        }
    }

    pub(crate) fn ticked(&self, group: &Group<'_>) -> Ticked {
        let importable: Vec<&&Offered> = group.offered.iter().filter(|offered| offered.importable()).collect();
        let picked = importable.iter().filter(|offered| self.has(offered)).count();
        match picked {
            0 => Ticked::None,
            n if n == importable.len() => Ticked::All,
            _ => Ticked::Some,
        }
    }

    pub(crate) fn toggle_group(&mut self, group: &Group<'_>) {
        let all = self.ticked(group) == Ticked::All;
        for offered in group.offered.iter().filter(|offered| offered.importable()) {
            if all {
                self.0.remove(&offered.persona.id);
            } else {
                self.0.insert(offered.persona.id.clone());
            }
        }
    }

    pub(crate) fn picked<'a>(&self, catalogue: &'a Catalogue) -> Vec<&'a Offered> {
        catalogue.personas.iter().filter(|offered| offered.importable() && self.has(offered)).collect()
    }
}

pub(crate) fn catalogue_args(source: &str) -> Vec<String> {
    let mut command = args(&["persona", "import", "--catalogue", "--json"]);
    command.extend(["--".into(), source.trim().to_string()]);
    command
}

pub(crate) fn only_args(source: &str, picked: &[&Offered]) -> Vec<String> {
    let mut command = args(&["persona", "import", "--yes", "--json"]);
    if picked.iter().any(|offered| offered.persona.replaced) {
        command.push("--replace".into());
    }
    command.extend(picked.iter().map(|offered| format!("--only={}", offered.persona.id)));
    command.extend(["--".into(), source.trim().to_string()]);
    command
}

pub(crate) fn confirmation(catalogue: &Catalogue, picked: &[&Offered]) -> String {
    let replaced: Vec<String> =
        picked.iter().filter(|offered| offered.persona.replaced).map(|offered| offered.persona.label()).collect();
    let mut text = sentence(&catalogue.check);
    if !replaced.is_empty() {
        text = format!("{text}\n\nIt replaces yours: {}.", and_list(&replaced));
    }
    text
}

#[cfg(test)]
mod tests;
