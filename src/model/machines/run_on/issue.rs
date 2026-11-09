use serde::Deserialize;

use super::checks::clipped;

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub(crate) struct IssueThere {
    #[serde(default)]
    pub prompt: String,
    pub issue: Named,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Named {
    pub branch: String,
}

impl IssueThere {
    pub(crate) fn task(&self) -> String {
        self.prompt.trim_end().to_string()
    }

    pub(crate) fn branch(&self) -> &str {
        &self.issue.branch
    }

    pub(crate) fn prompt_line(&self) -> Option<String> {
        Some(clipped(&self.prompt)).filter(|line| !line.is_empty())
    }
}

pub(crate) fn follow(typed: &str, was: &str, now: &str) -> Option<String> {
    let untouched = typed.trim() == was.trim();
    (untouched && typed.trim() != now.trim()).then(|| now.to_string())
}

#[cfg(test)]
mod tests;
