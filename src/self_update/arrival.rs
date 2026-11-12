use std::path::{Path, PathBuf};

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

use super::{NAME, release};
use crate::notify::update::UpdateToast;
use crate::prefs;
use crate::store::ToastKind;

const FILE: &str = "relaunch.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Awaited {
    pub version: String,
    pub program: PathBuf,
}

pub(crate) fn path(settings: &Path) -> PathBuf {
    prefs::beside(settings, FILE)
}

pub(crate) fn remember(path: &Path, awaited: &Awaited) -> anyhow::Result<()> {
    let mut text = serde_json::to_string_pretty(awaited).context("could not write the note")?;
    text.push('\n');
    prefs::write_atomic(path, &text)
}

pub(crate) fn take(path: &Path) -> Option<Awaited> {
    let text = std::fs::read_to_string(path).ok()?;
    let _ = std::fs::remove_file(path);
    serde_json::from_str(&text).ok()
}

pub(crate) fn toast(awaited: &str, current: &str, here: &str, there: &str) -> UpdateToast {
    if !release::newer(awaited, current) {
        return UpdateToast { text: format!("{NAME} is now {current}"), kind: ToastKind::Done, details: None };
    }
    let from = if here == there { String::new() } else { format!(" from {here}") };
    let text = format!("This window runs {NAME} {current}{from}, not {awaited}, which the update installed in {there}");
    UpdateToast { text, kind: ToastKind::Error, details: None }
}

#[cfg(test)]
mod tests;
