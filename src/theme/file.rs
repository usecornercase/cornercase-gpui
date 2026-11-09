use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub(crate) type Section = Map<String, Value>;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct ThemeFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extends: Option<String>,
    #[serde(skip_serializing_if = "Section::is_empty")]
    pub terminal: Section,
    #[serde(skip_serializing_if = "Section::is_empty")]
    pub chrome: Section,
    #[serde(skip_serializing_if = "Section::is_empty")]
    pub opacity: Section,
    #[serde(skip_serializing_if = "Section::is_empty")]
    pub shape: Section,
    #[serde(flatten)]
    pub other: Section,
}

pub(crate) fn parse(text: &str) -> Result<ThemeFile> {
    serde_json::from_str(text).context("the file is not a theme")
}

impl ThemeFile {
    pub(crate) fn text(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default() + "\n"
    }
}

#[cfg(test)]
mod tests;
