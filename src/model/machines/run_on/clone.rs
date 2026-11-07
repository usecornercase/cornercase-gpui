use std::time::{Duration, Instant};

use serde::Deserialize;

use super::super::seen::{MachineSeen, ThereId};
use super::checks::clipped;
use crate::model::GitRemote;
use crate::panels::files::disk::human_size;
use crate::platform::Os;
use crate::store::args;

pub(crate) const READING: &str = "Reading the git-ignored files here…";
pub(crate) const NO_FILES: &str = "No git-ignored files here.";
const OVER_THE_TOTAL: &str = "Over the 8 MiB a clone takes.";
const TOO_BIG: &str = "too big to copy";
const UNREADABLE: &str = "its answer could not be read";
const LISTED_WITHIN: Duration = Duration::from_secs(30);
const MOST_IN_ALL: u64 = 8 * 1024 * 1024;
const MOST_NAMED: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Cloned {
    pub machine: String,
    pub boot: String,
    pub project: u64,
    pub there: ThereId,
    pub at: Instant,
}

impl Cloned {
    pub(crate) fn of(&self, seen: &MachineSeen, project: u64) -> bool {
        self.machine == seen.name && self.boot == seen.boot && self.project == project
    }

    pub(crate) fn listing_soon(&self, now: Instant) -> bool {
        now < self.at + LISTED_WITHIN
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Unwritten {
    pub path: String,
    pub why: String,
}

impl Unwritten {
    fn named(&self, machine: &str) -> Option<String> {
        let path = clipped(&self.path);
        if path.is_empty() {
            return None;
        }
        let why = clipped(&self.why);
        let why = if why.is_empty() { format!("{machine} gave no reason") } else { why };
        Some(clipped(&format!("{path} ({why})")))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
struct CloneAnswer {
    project: Option<ThereId>,
    path: String,
    unwritten: Vec<Unwritten>,
}

pub(crate) fn parse_clone(out: &str) -> Result<(ThereId, Vec<Unwritten>), String> {
    let answer: CloneAnswer = serde_json::from_str(out.trim()).map_err(|_| UNREADABLE.to_string())?;
    let there = answer.project.filter(|_| !answer.path.trim().is_empty());
    there.map(|there| (there, answer.unwritten)).ok_or_else(|| UNREADABLE.to_string())
}

pub(crate) fn cloned_note(name: &str, machine: &str, unwritten: &[Unwritten]) -> String {
    let done = format!("Cloned {name} on {machine}");
    let left: Vec<String> = unwritten.iter().filter_map(|file| file.named(machine)).collect();
    if left.is_empty() {
        return done;
    }
    let named = left.iter().take(MOST_NAMED).cloned().collect::<Vec<_>>().join(", ");
    match left.len().saturating_sub(MOST_NAMED) {
        0 => format!("{done} · not copied: {named}"),
        more => format!("{done} · not copied: {named} … and {more} more"),
    }
}

pub(crate) fn intro(machine: &str, os: Os) -> String {
    if os == Os::Windows {
        return format!(
            "{machine} clones it from the address, without git-ignored files (such as .env): this computer runs Windows, where reading them to copy is not supported yet."
        );
    }
    format!(
        "{machine} clones it from the address, then copies the git-ignored files you tick (such as .env) into the clone. Ignored folders, such as node_modules, are never offered."
    )
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct IgnoredFile {
    pub path: String,
    pub size: u64,
    pub too_big: bool,
}

impl IgnoredFile {
    pub(crate) fn size_words(&self) -> String {
        let size = human_size(self.size);
        if self.too_big { format!("{size} · {TOO_BIG}") } else { size }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
struct Listing {
    files: Vec<IgnoredFile>,
    left_out: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum FileList {
    #[default]
    Reading,
    Failed(String),
    Read {
        files: Vec<(IgnoredFile, bool)>,
        left_out: usize,
    },
}

impl FileList {
    pub(crate) fn heard(answer: Result<String, String>) -> Self {
        let listing = answer.and_then(|out| serde_json::from_str(out.trim()).map_err(|_| UNREADABLE.to_string()));
        match listing {
            Ok(Listing { files, left_out }) => {
                Self::Read { files: files.into_iter().map(|file| (file, false)).collect(), left_out }
            }
            Err(why) => Self::Failed(why),
        }
    }

    pub(crate) fn offers_any(&self) -> bool {
        matches!(self, Self::Read { files, .. } if files.iter().any(|(file, _)| !file.too_big))
    }

    pub(crate) fn toggle(&mut self, at: usize) -> Result<(), String> {
        let Self::Read { files, .. } = self else { return Ok(()) };
        let total: u64 = files.iter().filter(|(_, ticked)| *ticked).map(|(file, _)| file.size).sum();
        let Some((file, ticked)) = files.get_mut(at).filter(|(file, _)| !file.too_big) else { return Ok(()) };
        if !*ticked && total + file.size > MOST_IN_ALL {
            return Err(OVER_THE_TOTAL.to_string());
        }
        *ticked = !*ticked;
        Ok(())
    }

    pub(crate) fn ticked(&self) -> Vec<String> {
        let Self::Read { files, .. } = self else { return Vec::new() };
        files.iter().filter(|(file, ticked)| *ticked && !file.too_big).map(|(file, _)| file.path.clone()).collect()
    }
}

pub(crate) fn not_offered(left_out: usize) -> String {
    format!("… and {left_out} more not offered (200 at most)")
}

pub(crate) fn ignored_args(from: &str) -> Vec<String> {
    vec!["clone".to_string(), "--list-ignored".to_string(), format!("--from={from}"), "--json".to_string()]
}

fn host_and_path(url: &str) -> bool {
    match url.split_once("://") {
        Some((_, rest)) => {
            let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
            let host = authority.rsplit('@').next().unwrap_or_default().split(':').next().unwrap_or_default();
            !host.is_empty() && !path.trim_matches('/').is_empty()
        }
        None => url
            .split_once(':')
            .is_some_and(|(authority, path)| !authority.is_empty() && !authority.contains('/') && !path.is_empty()),
    }
}

pub(crate) fn clone_address(remotes: &[GitRemote]) -> Option<&str> {
    let usable: Vec<&GitRemote> = remotes.iter().filter(|remote| host_and_path(&remote.url)).collect();
    let named = |name: &str| usable.iter().find(|remote| remote.name == name).copied();
    named("upstream").or_else(|| named("origin")).or_else(|| usable.first().copied()).map(|remote| remote.url.as_str())
}

fn is_local_path(url: &str) -> bool {
    if url.contains("://") {
        return false;
    }
    match (url.find(':'), url.find('/')) {
        (Some(colon), Some(slash)) => slash < colon,
        (Some(_), None) => false,
        (None, _) => true,
    }
}

fn naming_part(url: &str) -> &str {
    if is_local_path(url) {
        return url;
    }
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.split(['?', '#']).next().unwrap_or(rest)
}

pub(crate) fn folder_name(url: &str) -> Option<String> {
    let url = naming_part(url.trim()).trim_end_matches('/');
    let url = url.strip_suffix("/.git").unwrap_or(url).trim_end_matches('/');
    let last = url.rsplit(['/', ':']).next().unwrap_or_default();
    let name = last.strip_suffix(".git").or_else(|| last.strip_suffix(".bundle")).unwrap_or(last);
    match name {
        "" | "." | ".." => None,
        name if name.eq_ignore_ascii_case(".git") => None,
        name => Some(name.to_string()),
    }
}

pub(crate) fn destination(folder: &str, url: &str) -> String {
    let name = folder_name(url).unwrap_or_else(|| "…".to_string());
    let folder = folder.trim().trim_end_matches('/');
    if folder.is_empty() { format!("~/{name}") } else { format!("{folder}/{name}") }
}

pub(crate) fn clone_args(url: &str, folder: &str, files: &[String], from: &str) -> Vec<String> {
    let mut command = args(&["clone", url.trim()]);
    let folder = folder.trim();
    if !folder.is_empty() {
        command.push(folder.to_string());
    }
    command.extend(files.iter().map(|file| format!("--file={file}")));
    if !files.is_empty() {
        command.push(format!("--from={from}"));
    }
    command.push("--json".to_string());
    command
}

#[cfg(test)]
mod tests;
