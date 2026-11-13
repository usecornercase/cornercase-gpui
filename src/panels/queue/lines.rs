use gpui::{ClipboardItem, Context};

use super::QueuePanel;
use super::detail::View;
use crate::store::{Store, ToastKind};
use crate::ui::selection::{Edge, Selecting, Selection, Selects};

pub(super) fn selected_text(lines: &[String], selection: Selection<usize>) -> String {
    let (first, last) = selection.ends();
    let shown = (first..=last).filter_map(|ix| lines.get(ix).map(|line| (ix, line.as_str())));
    selection.text(shown, |ix, range| lines.get(ix).and_then(|line| line.get(range)).unwrap_or_default())
}

pub(super) fn picked(selection: Selection<usize>) -> String {
    let (first, last) = selection.covered();
    match last - first + 1 {
        1 => "1 line".to_string(),
        n => format!("{n} lines"),
    }
}

impl Selects for QueuePanel {
    type Spot = usize;

    fn selecting(&mut self) -> Option<&mut Selecting<usize>> {
        self.detail.as_mut().filter(|detail| detail.view == View::Text).map(|detail| &mut detail.selecting)
    }

    fn scroll_a_row(&mut self, edge: Edge) {
        if let Some(detail) = self.detail.as_ref() {
            detail.lists.of(detail.page, View::Text).scroll_by(edge.a_row());
        }
    }

    fn line_text(&self, spot: usize) -> Option<&str> {
        self.detail.as_ref()?.lines().get(spot).map(String::as_str)
    }

    fn selected_text(&self) -> Option<String> {
        let detail = self.detail.as_ref()?;
        Some(selected_text(detail.lines(), detail.selecting.selection?))
    }
}

impl QueuePanel {
    pub(crate) fn copy_selection(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(detail) = self.detail.as_ref() else { return false };
        let Some(selection) = detail.selecting.selection.filter(|s| !s.is_empty()) else { return false };
        let text = selected_text(detail.lines(), selection);
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        let what = format!("Copied {}", picked(selection));
        Store::get(cx).update(cx, |s, cx| s.toast(what, ToastKind::Info, cx));
        true
    }
}

#[cfg(test)]
mod tests;
