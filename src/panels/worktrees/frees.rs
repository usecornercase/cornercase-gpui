use std::fmt::Write as _;

use serde::Deserialize;

use super::data::{Build, MergedBuilds, Worktree, name_of};
use super::removal::{Said, Style};
use crate::cli::Version;
use crate::desktop;
use crate::panels::files::disk::human_size;
use crate::panels::flows::model::plural;
use crate::store::{Busy, ToastKind};

pub(super) const FREE_MERGED_SINCE: Version = Version::new(0, 13, 70);
pub(super) const CHECKING_MERGED: &str = "check-merged-builds";
pub(super) const FREEING_MERGED: &str = "free-merged-builds";

const FREEABLE: &str = "freeable";
const FREED: &str = "freed";
const REFUSED: &str = "refused";
const NO_WHY: &str = "cornercase did not say why";
const NOTHING_FREED: &str = "Nothing was freed";
const NOTHING_TO_FREE: &str = "No merged worktree has builds to free";
const PROGRAM_HINT: &str = "Where a program runs, Free builds… on that worktree's row can free it anyway.";
const FILTER_NOTE: &str = "This frees every worktree named here, including those the filter hides.";
const KEPT_FOLDER: &str = "it is no longer an ignored folder there, or it is on another filesystem";

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct FailedFolder {
    pub(super) path: String,
    pub(super) error: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Freed {
    pub(super) path: String,
    pub(super) bytes: u64,
    pub(super) folders: Vec<Build>,
    pub(super) kept: Vec<String>,
    pub(super) failed: Vec<FailedFolder>,
    pub(super) running: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Entry {
    outcome: String,
    why: Option<String>,
    #[serde(flatten)]
    freed: Freed,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(super) struct Merged {
    pub(super) bytes: u64,
    pub(super) worktrees: Vec<Entry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Goes,
    Kept,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Asked {
    Nothing(String),
    Cannot(String),
    Question { title: String, lines: Vec<Said>, job: Busy, confirmed: Vec<String> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MergedJob {
    Idle,
    Checking,
    Freeing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum FreeMerged {
    Hidden,
    Needs(String),
    Checking,
    Freeing,
    Rests,
    Offered,
}

impl Entry {
    fn outcome(&self) -> Outcome {
        match self.outcome.as_str() {
            FREEABLE | FREED => Outcome::Goes,
            REFUSED => Outcome::Kept,
            _ => Outcome::Failed,
        }
    }

    fn why(&self) -> &str {
        self.why.as_deref().unwrap_or(NO_WHY)
    }

    fn name(&self, worktrees: &[Worktree]) -> String {
        name_of(worktrees, &self.freed.path)
    }
}

impl Merged {
    fn going(&self) -> Vec<&Entry> {
        self.worktrees.iter().filter(|e| e.outcome() == Outcome::Goes).collect()
    }

    fn going_bytes(&self) -> u64 {
        self.going().iter().map(|e| e.freed.bytes).sum()
    }

    fn names_a_program(&self) -> bool {
        self.worktrees.iter().any(|e| e.outcome() == Outcome::Kept && !e.freed.running.is_empty())
    }
}

pub(super) fn checking(path: &str) -> String {
    format!("check-builds:{path}")
}

pub(super) fn freeing(path: &str) -> String {
    format!("free-builds:{path}")
}

pub(super) fn free_command(path: &str, check: bool, force: bool) -> Vec<String> {
    let mut command: Vec<String> = ["worktrees", "free-builds", "--path", path, "--json"].map(str::to_string).to_vec();
    if check {
        command.push("--check".into());
    }
    if force {
        command.push("--force".into());
    }
    command
}

pub(super) fn free_merged_command(check: bool, project: Option<u64>) -> Vec<String> {
    let mut command: Vec<String> = ["worktrees", "free-builds", "--merged", "--json"].map(str::to_string).to_vec();
    if check {
        command.push("--check".into());
    }
    if let Some(project) = project {
        command.extend(["--project".to_string(), project.to_string()]);
    }
    command
}

pub(super) fn free_title(name: &str) -> String {
    format!("Free the build folders of {name}?")
}

pub(super) fn freeing_label(name: &str) -> String {
    format!("Freeing the builds of {name}…")
}

fn folders_text(freed: &Freed) -> String {
    let folders: Vec<String> = freed.folders.iter().map(|f| format!("{} {}", f.path, human_size(f.bytes))).collect();
    format!("{}: {}", human_size(freed.bytes), folders.join(", "))
}

fn what_stays(several: bool) -> &'static str {
    if several {
        "The worktrees, their branches and their changes stay; files inside these folders are lost, \
         and each project's setup can rebuild them."
    } else {
        "The worktree, its branch and its changes stay; files inside these folders are lost, \
         and the project's setup can rebuild them."
    }
}

pub(super) fn free_message(freed: &Freed) -> String {
    let mut text = format!("Frees {}. {}", folders_text(freed), what_stays(false));
    if !freed.running.is_empty() {
        let _ = write!(text, " {} runs in it and may still use them.", freed.running.join(", "));
    }
    text
}

fn folder_problems(freed: &Freed, name: &str) -> Vec<String> {
    let failed = freed.failed.iter().map(|f| format!("Could not free {} in {name}: {}", f.path, f.error));
    let kept = freed.kept.iter().map(|folder| format!("Kept {folder} in {name}: {KEPT_FOLDER}"));
    failed.chain(kept).collect()
}

fn toast_of(first: String, problems: Vec<String>) -> (String, ToastKind) {
    let kind = if problems.is_empty() { ToastKind::Info } else { ToastKind::Error };
    let lines: Vec<String> = std::iter::once(first).chain(problems).collect();
    (lines.join("\n"), kind)
}

pub(super) fn row_toast(freed: &Freed, name: &str) -> (String, ToastKind) {
    let first = if freed.bytes == 0 { NOTHING_FREED.to_string() } else { format!("Freed {}", human_size(freed.bytes)) };
    toast_of(first, folder_problems(freed, name))
}

fn entry_problems(entry: &Entry, name: &str) -> Vec<String> {
    match entry.outcome() {
        Outcome::Goes => folder_problems(&entry.freed, name),
        Outcome::Kept => vec![format!("Did not free {name}: {}", entry.why())],
        Outcome::Failed => vec![format!("Could not free {name}: {}", entry.why())],
    }
}

fn unlisted(answer: &Merged, worktrees: &[Worktree], confirmed: &[String]) -> Vec<String> {
    let goes = answer.going().into_iter().filter(|e| e.freed.bytes > 0);
    let unlisted = goes.filter(|e| !confirmed.contains(&e.freed.path));
    unlisted.map(|e| format!("Also freed {}, which the question did not offer to free", e.name(worktrees))).collect()
}

pub(super) fn merged_toast(answer: &Merged, worktrees: &[Worktree], confirmed: &[String]) -> (String, ToastKind) {
    if answer.worktrees.is_empty() {
        return (NOTHING_TO_FREE.to_string(), ToastKind::Info);
    }
    let freed = answer.going().iter().filter(|e| e.freed.bytes > 0).count();
    let first = if freed == 0 {
        NOTHING_FREED.to_string()
    } else {
        format!("Freed {} in {}", human_size(answer.going_bytes()), plural(freed, "merged worktree"))
    };
    let mut problems: Vec<String> =
        answer.worktrees.iter().flat_map(|entry| entry_problems(entry, &entry.name(worktrees))).collect();
    problems.extend(unlisted(answer, worktrees, confirmed));
    toast_of(first, problems)
}

fn entry_lines(entry: &Entry, name: String) -> Vec<Said> {
    match entry.outcome() {
        Outcome::Goes => vec![(name, Style::Heading), (format!("frees {}", folders_text(&entry.freed)), Style::Line)],
        Outcome::Kept => vec![(name, Style::Risky), (format!("keeps its builds: {}", entry.why()), Style::Lost)],
        Outcome::Failed => vec![(name, Style::Risky), (format!("could not be measured: {}", entry.why()), Style::Lost)],
    }
}

fn question_lines(answer: &Merged, worktrees: &[Worktree], filtering: bool) -> Vec<Said> {
    let mut lines = Vec::new();
    for outcome in [Outcome::Goes, Outcome::Kept, Outcome::Failed] {
        for entry in answer.worktrees.iter().filter(|e| e.outcome() == outcome) {
            lines.extend(entry_lines(entry, entry.name(worktrees)));
        }
    }
    lines.push((what_stays(answer.going().len() > 1).to_string(), Style::Note));
    if answer.names_a_program() {
        lines.push((PROGRAM_HINT.to_string(), Style::Note));
    }
    if filtering {
        lines.push((FILTER_NOTE.to_string(), Style::Note));
    }
    lines
}

fn question_title(answer: &Merged, worktrees: &[Worktree]) -> (String, String) {
    match answer.going().as_slice() {
        [one] => (free_title(&one.name(worktrees)), freeing_label(&one.name(worktrees))),
        many => (
            format!(
                "Free {} of builds in {}?",
                human_size(answer.going_bytes()),
                plural(many.len(), "merged worktree")
            ),
            freeing_label(&plural(many.len(), "merged worktree")),
        ),
    }
}

pub(super) fn ask_merged(answer: &Merged, worktrees: &[Worktree], filtering: bool) -> Asked {
    if answer.worktrees.is_empty() {
        return Asked::Nothing(NOTHING_TO_FREE.to_string());
    }
    if answer.going().is_empty() {
        let mut lines: Vec<String> = answer
            .worktrees
            .iter()
            .map(|entry| format!("Cannot free {}: {}", entry.name(worktrees), entry.why()))
            .collect();
        lines.extend(answer.names_a_program().then(|| PROGRAM_HINT.to_string()));
        return Asked::Cannot(lines.join("\n"));
    }
    let (title, label) = question_title(answer, worktrees);
    Asked::Question {
        title,
        lines: question_lines(answer, worktrees, filtering),
        job: Busy::new(FREEING_MERGED, label),
        confirmed: answer.going().iter().map(|e| e.freed.path.clone()).collect(),
    }
}

pub(super) fn merged_job(busy: impl Fn(&str) -> bool) -> MergedJob {
    if busy(FREEING_MERGED) {
        MergedJob::Freeing
    } else if busy(CHECKING_MERGED) {
        MergedJob::Checking
    } else {
        MergedJob::Idle
    }
}

fn needs_note(here: Option<Version>) -> String {
    let has = here.map(|v| format!(" (it has {v})")).unwrap_or_default();
    format!(
        "Freeing the merged worktrees' builds in one go needs cornercase {FREE_MERGED_SINCE} or newer on this {}{has}.",
        desktop::here()
    )
}

pub(super) fn free_merged(
    merged: Option<MergedBuilds>,
    job: MergedJob,
    rows_busy: bool,
    here: Option<Version>,
) -> FreeMerged {
    match (job, merged) {
        (MergedJob::Checking, _) => FreeMerged::Checking,
        (MergedJob::Freeing, _) => FreeMerged::Freeing,
        (MergedJob::Idle, None) => FreeMerged::Hidden,
        (MergedJob::Idle, Some(_)) if !FREE_MERGED_SINCE.reached_by(here) => FreeMerged::Needs(needs_note(here)),
        (MergedJob::Idle, Some(_)) if rows_busy => FreeMerged::Rests,
        (MergedJob::Idle, Some(_)) => FreeMerged::Offered,
    }
}

#[cfg(test)]
mod tests;
