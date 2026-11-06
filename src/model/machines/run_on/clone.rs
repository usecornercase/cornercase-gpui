use serde::Deserialize;

use super::super::seen::{MachineSeen, ThereId};
use crate::model::GitRemote;
use crate::store::args;

pub(crate) const FILES_NOTE: &str =
    "Git-ignored files such as .env stay on this computer: copying them needs usecornercase/cornercase-terminal#678.";
const UNREADABLE: &str = "its answer could not be read";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Cloned {
    pub machine: String,
    pub boot: String,
    pub project: u64,
    pub there: ThereId,
}

impl Cloned {
    pub(crate) fn of(&self, seen: &MachineSeen, project: u64) -> bool {
        self.machine == seen.name && self.boot == seen.boot && self.project == project
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct CloneAnswer {
    pub project: Option<ThereId>,
    pub path: String,
}

pub(crate) fn parse_clone(out: &str) -> Result<ThereId, String> {
    let answer: CloneAnswer = serde_json::from_str(out.trim()).map_err(|_| UNREADABLE.to_string())?;
    answer.project.filter(|_| !answer.path.trim().is_empty()).ok_or_else(|| UNREADABLE.to_string())
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

pub(crate) fn clone_args(url: &str, folder: &str) -> Vec<String> {
    let mut command = args(&["clone", url.trim()]);
    let folder = folder.trim();
    if !folder.is_empty() {
        command.push(folder.to_string());
    }
    command.push("--json".to_string());
    command
}

#[cfg(test)]
mod tests;
