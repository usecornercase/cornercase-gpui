use serde::Deserialize;
use serde_json::Value;

use super::{Session, ThereProject, status_rank};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Section {
    Projects,
    Workspaces,
    Agents,
}

impl Section {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Projects => "projects",
            Self::Workspaces => "workspaces",
            Self::Agents => "agents",
        }
    }

    pub(crate) const fn title(self) -> &'static str {
        match self {
            Self::Projects => "Projects",
            Self::Workspaces => "Workspaces",
            Self::Agents => "Agents",
        }
    }

    fn named(name: &str) -> Option<Self> {
        [Self::Projects, Self::Workspaces, Self::Agents].into_iter().find(|section| section.name() == name)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(from = "Option<Vec<Value>>")]
pub(crate) struct SectionFolds {
    pub projects: bool,
    pub workspaces: bool,
    pub agents: bool,
}

impl From<Option<Vec<Value>>> for SectionFolds {
    fn from(listed: Option<Vec<Value>>) -> Self {
        let mut folds = Self::default();
        for section in listed.into_iter().flatten().filter_map(|entry| entry.as_str().and_then(Section::named)) {
            *folds.flag(section) = true;
        }
        folds
    }
}

impl SectionFolds {
    pub(crate) const fn folded(self, section: Section) -> bool {
        match section {
            Section::Projects => self.projects,
            Section::Workspaces => self.workspaces,
            Section::Agents => self.agents,
        }
    }

    pub(crate) fn flag(&mut self, section: Section) -> &mut bool {
        match section {
            Section::Projects => &mut self.projects,
            Section::Workspaces => &mut self.workspaces,
            Section::Agents => &mut self.agents,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct SectionTags<'a> {
    pub count: usize,
    pub attention: Option<&'a str>,
}

pub(super) fn loudest<'a>(statuses: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    statuses.max_by_key(|status| status_rank(Some(status)))
}

impl Session {
    pub(crate) fn projects_tags(&self) -> SectionTags<'_> {
        let ours = self.projects.iter().map(|project| self.attention_under(project));
        let alone = self.alone().map(|(_, there)| there.attention());
        let rows: Vec<_> = ours.chain(alone).collect();
        SectionTags { count: rows.len(), attention: loudest(rows.into_iter().flatten()) }
    }

    pub(crate) fn workspaces_tags(&self, project: u64) -> SectionTags<'_> {
        let Some(project) = self.projects.iter().find(|p| p.id == project) else {
            return SectionTags::default();
        };
        SectionTags { count: self.workspaces_under(project), attention: self.attention_under(project) }
    }
}

impl ThereProject {
    pub(crate) fn tags(&self) -> SectionTags<'_> {
        SectionTags { count: self.workspaces.len(), attention: self.attention() }
    }
}

#[cfg(test)]
mod tests;
