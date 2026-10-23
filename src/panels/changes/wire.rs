use serde::Deserialize;

use crate::model::Style;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Mode {
    #[default]
    Uncommitted,
    Commits,
    All,
}

impl Mode {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Uncommitted => "uncommitted",
            Self::Commits => "commits",
            Self::All => "all",
        }
    }

    pub(super) fn uncommitted(self) -> bool {
        self == Self::Uncommitted
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum State {
    #[default]
    Loading,
    Ready,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Staging {
    No,
    Partly,
    Yes,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum FileStatus {
    #[default]
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
    #[serde(other)]
    Other,
}

impl FileStatus {
    pub(super) fn letter(self) -> &'static str {
        match self {
            Self::Modified => "M",
            Self::Added => "A",
            Self::Deleted => "D",
            Self::Renamed => "R",
            Self::Untracked => "U",
            Self::Other => "?",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum FoldClass {
    #[default]
    Open,
    Lockfile,
    Binary,
    Large,
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct FileInfo {
    pub path: String,
    pub old_path: Option<String>,
    pub status: FileStatus,
    pub added: usize,
    pub removed: usize,
    pub fold: FoldClass,
    pub viewed: bool,
    pub staged: Option<Staging>,
    pub digest: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct WorkspaceChanges {
    pub project: u64,
    pub workspace: u64,
    pub base: Option<String>,
    pub added: usize,
    pub removed: usize,
    pub files: Vec<FileInfo>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct Summary {
    pub revision: u64,
    pub scope: String,
    pub mode: Mode,
    pub state: State,
    pub error: Option<String>,
    pub workspaces: Vec<WorkspaceChanges>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum LineKind {
    #[default]
    Context,
    Removed,
    Added,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct Segment {
    pub style: usize,
    pub text: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct Line {
    pub kind: LineKind,
    pub old: Option<u32>,
    pub new: Option<u32>,
    pub text: String,
    pub emphasis: Vec<[usize; 2]>,
    pub syntax: Option<Vec<Segment>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct Hunk {
    pub old_start: u32,
    pub new_start: u32,
    pub context: String,
    pub lines: Vec<Line>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(super) struct Patch {
    pub workspace: u64,
    pub mode: Mode,
    #[serde(flatten)]
    pub file: FileInfo,
    pub styles: Vec<Style>,
    pub hunks: Vec<Hunk>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct GapLine {
    pub old: u32,
    pub new: u32,
    pub text: String,
    pub syntax: Option<Vec<Segment>>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(super) struct Gap {
    pub workspace: u64,
    pub path: String,
    pub hunk: usize,
    pub styles: Vec<Style>,
    pub lines: Vec<GapLine>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(super) struct Base {
    pub workspace: u64,
    #[serde(rename = "base")]
    pub chosen: Option<String>,
    pub default: Option<String>,
    pub branches: Vec<String>,
}

pub(super) fn summary(line: &str) -> serde_json::Result<Summary> {
    serde_json::from_str(line)
}

#[cfg(test)]
mod tests;
