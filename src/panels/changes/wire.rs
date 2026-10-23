//! What `cornercase changes` prints with `--json` (and `list --follow`, one summary per line):
//! summaries of changed files, one file's patch, the unchanged lines of a gap and the bases.

use serde::Deserialize;

use crate::model::Style;

/// What the panel compares; `changes --mode`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Uncommitted,
    Commits,
    All,
}

impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Self::Uncommitted => "uncommitted",
            Self::Commits => "commits",
            Self::All => "all",
        }
    }

    /// Whether the files can be staged or discarded: only changes not committed yet can.
    pub fn uncommitted(self) -> bool {
        self == Self::Uncommitted
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    #[default]
    Loading,
    Ready,
    Failed,
}

/// How much of a file is in git's index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Staging {
    No,
    Partly,
    Yes,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileStatus {
    #[default]
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
    /// A status a newer cornercase added.
    #[serde(other)]
    Other,
}

impl FileStatus {
    /// The letter the panel shows before the path.
    pub fn letter(self) -> &'static str {
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

/// Why a file's lines are hidden at first: a lockfile, a binary file, or too many lines.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FoldClass {
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
pub struct FileInfo {
    pub path: String,
    pub old_path: Option<String>,
    pub status: FileStatus,
    pub added: usize,
    pub removed: usize,
    pub fold: FoldClass,
    pub viewed: bool,
    /// Present in Uncommitted and Every project only.
    pub staged: Option<Staging>,
    /// The file's review identity, 16 hex digits: it changes whenever the file's change does.
    pub digest: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct WorkspaceChanges {
    pub project: u64,
    pub workspace: u64,
    /// The branch Commits and All compare with, when one was chosen or found.
    pub base: Option<String>,
    pub added: usize,
    pub removed: usize,
    pub files: Vec<FileInfo>,
}

/// One line of `changes list --json` / `--follow`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Summary {
    pub revision: u64,
    pub scope: String,
    pub mode: Mode,
    pub state: State,
    pub error: Option<String>,
    pub workspaces: Vec<WorkspaceChanges>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LineKind {
    #[default]
    Context,
    Removed,
    Added,
}

/// A run of highlighted text: an index into the patch's `styles`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Segment {
    pub style: usize,
    pub text: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Line {
    pub kind: LineKind,
    pub old: Option<u32>,
    pub new: Option<u32>,
    pub text: String,
    /// `[start, end)` in characters of `text`: the words that changed.
    pub emphasis: Vec<[usize; 2]>,
    pub syntax: Option<Vec<Segment>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Hunk {
    pub old_start: u32,
    pub new_start: u32,
    pub context: String,
    pub lines: Vec<Line>,
}

/// `changes file PATH --json`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Patch {
    pub workspace: u64,
    pub mode: Mode,
    #[serde(flatten)]
    pub file: FileInfo,
    pub styles: Vec<Style>,
    pub hunks: Vec<Hunk>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct GapLine {
    pub old: u32,
    pub new: u32,
    pub text: String,
    pub syntax: Option<Vec<Segment>>,
}

/// `changes gap PATH --hunk N --json`: the unchanged lines before hunk N (from 1).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Gap {
    pub workspace: u64,
    pub path: String,
    pub hunk: usize,
    pub styles: Vec<Style>,
    pub lines: Vec<GapLine>,
}

/// `changes base --json`: the branches Commits and All can compare with.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Base {
    pub workspace: u64,
    pub base: Option<String>,
    pub default: Option<String>,
    pub branches: Vec<String>,
}

pub fn summary(line: &str) -> serde_json::Result<Summary> {
    serde_json::from_str(line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Colour;

    #[test]
    fn reads_a_summary_line() {
        let line = r#"{"boot":"1-2","error":null,"mode":"uncommitted","revision":3,"scope":"project","state":"ready","stream":1,
            "workspaces":[{"added":7,"base":null,"project":29,"removed":5,"workspace":28,"files":[
              {"added":0,"digest":"6113471e8352b284","fold":"open","old_path":null,"path":"gone.txt","removed":1,"staged":"no","status":"deleted","viewed":false},
              {"added":0,"digest":"c54c477b3900e220","fold":"open","old_path":"rename_me.txt","path":"renamed.txt","removed":0,"staged":"yes","status":"renamed","viewed":true},
              {"added":2,"digest":"a5c2fcc5bb9cebb1","fold":"binary","path":"x.png","removed":0,"status":"untracked","viewed":false}]}]}"#;
        let s = summary(line).expect("parses");
        assert_eq!(s.revision, 3);
        assert_eq!(s.state, State::Ready);
        assert_eq!(s.mode, Mode::Uncommitted);
        let w = &s.workspaces[0];
        assert_eq!((w.project, w.workspace, w.added, w.removed), (29, 28, 7, 5));
        assert_eq!(w.files[0].status, FileStatus::Deleted);
        assert_eq!(w.files[0].staged, Some(Staging::No));
        assert_eq!(w.files[1].old_path.as_deref(), Some("rename_me.txt"));
        assert!(w.files[1].viewed);
        assert_eq!(w.files[2].fold, FoldClass::Binary);
        assert_eq!(w.files[2].staged, None);
    }

    #[test]
    fn unknown_values_do_not_break_a_summary() {
        let line = r#"{"state":"loading","mode":"commits","workspaces":[{"files":[{"path":"a","status":"copied","fold":"huge","new_field":1}]}]}"#;
        let s = summary(line).expect("parses");
        assert_eq!(s.state, State::Loading);
        assert_eq!(s.mode, Mode::Commits);
        assert_eq!(s.workspaces[0].files[0].status, FileStatus::Other);
        assert_eq!(s.workspaces[0].files[0].fold, FoldClass::Other);
    }

    #[test]
    fn reads_a_failed_summary() {
        let s = summary(r#"{"state":"failed","error":"not a git repository","workspaces":[]}"#).expect("parses");
        assert_eq!(s.state, State::Failed);
        assert_eq!(s.error.as_deref(), Some("not a git repository"));
    }

    #[test]
    fn reads_a_patch_with_styles_and_emphasis() {
        let json = r#"{"added":1,"digest":"550a7a517a389403","fold":"open","mode":"uncommitted","old_path":null,
            "path":"src/main.rs","removed":1,"staged":"partly","status":"modified","viewed":false,"workspace":28,
            "styles":[{},{"fg":{"index":5}},{"fg":{"rgb":[1,2,3]},"bold":true}],
            "hunks":[{"context":"fn main","new_start":1,"old_start":1,"lines":[
              {"kind":"context","new":1,"old":1,"text":"fn main() {","syntax":[{"style":1,"text":"fn"},{"style":0,"text":" main() {"}]},
              {"kind":"removed","new":null,"old":2,"text":"    a","emphasis":[[4,5]]},
              {"kind":"added","new":2,"old":null,"text":"    b","emphasis":[[4,5]]}]}]}"#;
        let p: Patch = serde_json::from_str(json).expect("parses");
        assert_eq!(p.workspace, 28);
        assert_eq!(p.file.path, "src/main.rs");
        assert_eq!(p.file.staged, Some(Staging::Partly));
        assert_eq!(p.styles[1].fg, Some(Colour::Index(5)));
        assert_eq!(p.styles[2].fg, Some(Colour::Rgb([1, 2, 3])));
        assert!(p.styles[2].bold);
        let h = &p.hunks[0];
        assert_eq!((h.old_start, h.new_start, h.context.as_str()), (1, 1, "fn main"));
        assert_eq!(h.lines[1].kind, LineKind::Removed);
        assert_eq!(h.lines[1].new, None);
        assert_eq!(h.lines[2].emphasis, vec![[4, 5]]);
        assert_eq!(h.lines[0].syntax.as_ref().map(Vec::len), Some(2));
    }

    #[test]
    fn reads_a_gap_and_bases() {
        let gap: Gap = serde_json::from_str(
            r#"{"workspace":1,"path":"a.rs","hunk":2,"styles":[{}],"lines":[{"old":4,"new":5,"text":"x"}]}"#,
        )
        .expect("parses");
        assert_eq!((gap.hunk, gap.lines[0].old, gap.lines[0].new), (2, 4, 5));
        let base: Base =
            serde_json::from_str(r#"{"base":null,"branches":["feature","main"],"default":"main","workspace":28}"#)
                .expect("parses");
        assert_eq!(base.default.as_deref(), Some("main"));
        assert_eq!(base.branches, vec!["feature", "main"]);
    }
}
