use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use gpui::{App, Context};
use serde_json::{Value, json};

use super::{Browser, chrome};
use crate::store::{Busy, Store, ToastKind};

pub(crate) struct Download {
    name: String,
    job: u64,
}

#[derive(Default)]
pub(crate) struct Downloads {
    list: HashMap<String, Download>,
}

pub(crate) fn safe_name(suggested: &str) -> String {
    let name = suggested.rsplit(['/', '\\']).next().unwrap_or_default().trim();
    if name.is_empty() || name == "." || name == ".." { "download".to_string() } else { name.to_string() }
}

fn split_name(name: &str) -> (&str, &str) {
    let tar = name.find(".tar.").filter(|at| *at > 0);
    let dot = tar.or_else(|| name.rfind('.').filter(|at| *at > 0));
    dot.map_or((name, ""), |at| name.split_at(at))
}

pub(crate) fn free_path(dir: &Path, name: &str, taken: impl Fn(&Path) -> bool) -> PathBuf {
    let (stem, extension) = split_name(name);
    (0..10_000)
        .map(|n| if n == 0 { dir.join(name) } else { dir.join(format!("{stem} ({n}){extension}")) })
        .find(|path| !taken(path))
        .unwrap_or_else(|| dir.join(name))
}

fn home() -> PathBuf {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_default()
}

fn downloads_dir() -> PathBuf {
    home().join("Downloads")
}

fn staging() -> PathBuf {
    chrome::profile().join("downloads")
}

pub(crate) fn shown_dir(dir: &Path, home: &Path) -> String {
    match dir.strip_prefix(home) {
        Ok(rest) if !home.as_os_str().is_empty() => format!("~/{}", rest.display()),
        _ => dir.display().to_string(),
    }
}

fn move_to_downloads(staged: &Path, name: &str) -> io::Result<PathBuf> {
    let dir = downloads_dir();
    fs::create_dir_all(&dir)?;
    let target = free_path(&dir, name, Path::exists);
    if fs::rename(staged, &target).is_err() {
        fs::copy(staged, &target)?;
        fs::remove_file(staged)?;
    }
    Ok(target)
}

fn end(download: &Download, toast: Option<(String, ToastKind)>, cx: &mut App) {
    Store::get(cx).update(cx, |s, cx| {
        s.end_job(download.job, cx);
        if let Some((text, kind)) = toast {
            s.toast(text, kind, cx);
        }
    });
}

fn finish(moved: io::Result<PathBuf>, download: &Download, cx: &mut App) {
    let toast = match moved {
        Ok(path) => {
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let dir = path.parent().map(|d| shown_dir(d, &home())).unwrap_or_default();
            Some((format!("Downloaded {name} to {dir}"), ToastKind::Info))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => Some((
            format!("{} was downloaded but could not be moved to Downloads: {e}", download.name),
            ToastKind::Error,
        )),
    };
    end(download, toast, cx);
}

impl Browser {
    pub(super) fn allow_downloads(&self) {
        let staging = staging();
        let _ = fs::create_dir_all(&staging);
        let params =
            json!({ "behavior": "allowAndName", "downloadPath": staging.display().to_string(), "eventsEnabled": true });
        self.send(None, "Browser.setDownloadBehavior", params);
    }

    pub(super) fn download_begins(&mut self, p: &Value, cx: &mut Context<Self>) {
        let Some(guid) = p["guid"].as_str() else { return };
        let name = safe_name(p["suggestedFilename"].as_str().unwrap_or_default());
        let busy = Busy::new(format!("download:{guid}"), format!("Downloading {name}…"));
        let job = Store::get(cx).update(cx, |s, cx| s.start_job(busy, cx));
        self.notices.downloads.list.insert(guid.to_string(), Download { name, job });
    }

    pub(super) fn download_progress(&mut self, p: &Value, cx: &mut Context<Self>) {
        let (Some(guid), Some(state)) = (p["guid"].as_str(), p["state"].as_str()) else { return };
        if state == "inProgress" {
            return;
        }
        let Some(download) = self.notices.downloads.list.remove(guid) else { return };
        if state != "completed" {
            let text = format!("Downloading {} failed or was cancelled", download.name);
            end(&download, Some((text, ToastKind::Error)), cx);
            return;
        }
        let staged = p["filePath"].as_str().map_or_else(|| staging().join(guid), PathBuf::from);
        cx.spawn(async move |this, cx| {
            let name = download.name.clone();
            let moved = cx.background_executor().spawn(async move { move_to_downloads(&staged, &name) }).await;
            let _ = this.update(cx, |_, cx| finish(moved, &download, cx));
        })
        .detach();
    }
}

#[cfg(test)]
mod tests;
