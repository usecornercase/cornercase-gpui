use gpui::{App, FontWeight, IntoElement, ParentElement as _, Rgba, Styled as _, div, px};
use serde::de::DeserializeOwned;

use crate::model::{Colours, Style};
use crate::store::{Store, ToastKind};
use crate::theme::{FONT_FAMILY, Painter, colours};
use crate::ui;

use super::Look;
use super::wire::{FileInfo, FileStatus, Summary};

pub(super) fn find<'a>(summary: &'a Summary, workspace: u64, path: &str) -> Option<&'a FileInfo> {
    summary.workspaces.iter().filter(|w| w.workspace == workspace).flat_map(|w| &w.files).find(|f| f.path == path)
}

pub(super) fn find_mut<'a>(summary: &'a mut Summary, workspace: u64, path: &str) -> Option<&'a mut FileInfo> {
    summary
        .workspaces
        .iter_mut()
        .filter(|w| w.workspace == workspace)
        .flat_map(|w| &mut w.files)
        .find(|f| f.path == path)
}

pub(super) fn parse<T: DeserializeOwned>(result: anyhow::Result<String>) -> anyhow::Result<T> {
    result.and_then(|out| serde_json::from_str::<T>(&out).map_err(anyhow::Error::from))
}

pub(super) fn looks(styles: &[Style]) -> Vec<Look> {
    let colours = Colours::default();
    let painter = Painter { colours: &colours };
    styles.iter().map(|style| Look { fg: painter.resolve(style).0, bold: style.bold, italic: style.italic }).collect()
}

pub(super) fn toast(text: impl Into<String>, kind: ToastKind, cx: &mut App) {
    let text = text.into();
    Store::get(cx).update(cx, |s, cx| s.toast(text, kind, cx));
}

pub(super) fn split_path(path: &str) -> (String, String) {
    match path.rsplit_once('/') {
        Some((dir, name)) => (format!("{dir}/"), name.to_string()),
        None => (String::new(), path.to_string()),
    }
}

pub(super) fn plural(n: usize, word: &str) -> String {
    if n == 1 { format!("1 {word}") } else { format!("{n} {word}s") }
}

fn status_colour(status: FileStatus) -> Rgba {
    match status {
        FileStatus::Modified => colours::working(),
        FileStatus::Added | FileStatus::Untracked => colours::done(),
        FileStatus::Deleted => colours::failed(),
        FileStatus::Renamed => colours::accent(),
        FileStatus::Other => colours::muted(),
    }
}

pub(super) fn status_letter(status: FileStatus) -> gpui::Div {
    div()
        .font_family(FONT_FAMILY)
        .text_size(px(11.))
        .font_weight(FontWeight::BOLD)
        .text_color(status_colour(status))
        .child(status.letter())
}

pub(super) fn faint_detail(text: String) -> gpui::Div {
    div().flex_1().min_w(px(0.)).text_size(px(11.)).text_color(colours::faint()).child(ui::one_line(text))
}

pub(super) fn counts(added: usize, removed: usize) -> impl IntoElement {
    div()
        .flex_none()
        .flex()
        .gap(px(4.))
        .font_family(FONT_FAMILY)
        .text_size(px(11.))
        .child(div().text_color(colours::done()).child(format!("+{added}")))
        .child(div().text_color(colours::failed()).child(format!("−{removed}")))
}
