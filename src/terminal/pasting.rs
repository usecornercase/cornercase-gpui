use std::path::{Path, PathBuf};

use gpui::{ClipboardEntry, Context, ExternalPaths, Image, ImageFormat, Window};
use serde_json::json;

use crate::app::Root;
use crate::render::layout::Hit;

fn image_name(format: ImageFormat) -> String {
    let extension = match format {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpg",
        ImageFormat::Webp => "webp",
        ImageFormat::Gif => "gif",
        ImageFormat::Svg => "svg",
        ImageFormat::Bmp => "bmp",
        ImageFormat::Tiff => "tiff",
    };
    format!("pasted.{extension}")
}

fn image_args(pane: u64, format: ImageFormat) -> Vec<String> {
    let pane = pane.to_string();
    ["paste-file", "--pane", &pane, "--name", &image_name(format), "-"].map(String::from).to_vec()
}

fn file_args(pane: u64, path: &Path) -> Vec<String> {
    vec!["paste-file".into(), "--pane".into(), pane.to_string(), path.to_string_lossy().into_owned()]
}

fn shell_word(text: &str) -> String {
    let plain = !text.is_empty() && text.chars().all(|c| c.is_ascii_alphanumeric() || "/._-+,:@%=~".contains(c));
    if plain { text.to_string() } else { format!("'{}'", text.replace('\'', r"'\''")) }
}

fn paths_text(paths: &[PathBuf]) -> String {
    paths.iter().map(|path| shell_word(&path.to_string_lossy()) + " ").collect()
}

fn image_of(entries: &[ClipboardEntry]) -> Option<&Image> {
    entries.iter().find_map(|entry| match entry {
        ClipboardEntry::Image(image) => Some(image),
        ClipboardEntry::String(_) => None,
    })
}

impl Root {
    pub(crate) fn paste(&self, pane: u64, cx: &mut Context<Self>) {
        let Some(item) = cx.read_from_clipboard() else { return };
        if let Some(text) = item.text() {
            self.queue.input(pane, json!({ "paste": text }));
        } else if let Some(image) = image_of(item.entries()) {
            self.queue.cmd_with_stdin(image_args(pane, image.format), image.bytes.clone());
        }
    }

    pub(super) fn drop_paths(&mut self, paths: &ExternalPaths, window: &mut Window, _cx: &mut Context<Self>) {
        let under = match self.hit(window.mouse_position()) {
            Some(Hit::Pane(pane, _)) => Some(pane),
            _ => None,
        };
        let Some(pane) = under.or_else(|| self.active_pane()) else { return };
        if self.host.is_none() {
            self.queue.input(pane, json!({ "paste": paths_text(paths.paths()) }));
            return;
        }
        for path in paths.paths() {
            self.queue.cmd(file_args(pane, path));
        }
    }
}

#[cfg(test)]
mod tests;
