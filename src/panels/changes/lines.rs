use gpui::{AnyElement, ClickEvent, ClipboardItem, Context, IntoElement, StatefulInteractiveElement as _};

use crate::store::{Store, ToastKind};
use crate::ui::selection::{self, Edge, Selecting, Selection, Selects};

use super::diff::{self, Part, Spot};
use super::pieces::toast;
use super::wire::FileStatus;
use super::{ChangesPanel, DiffData};

const NO_OWN_TEXT: &str = "tabs may have become spaces: needs usecornercase/cornercase-terminal#667";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Numbered<'a> {
    old: Option<u32>,
    new: Option<u32>,
    text: &'a str,
}

impl DiffData {
    pub(super) fn line_at(&self, spot: Spot) -> Option<Numbered<'_>> {
        match spot.part {
            Part::Gap => {
                let line = self.gaps.get(&spot.hunk)?.0.lines.get(spot.line)?;
                Some(Numbered { old: Some(line.old), new: Some(line.new), text: &line.text })
            }
            Part::Patch => {
                let line = self.patch.hunks.get(spot.hunk)?.lines.get(spot.line)?;
                Some(Numbered { old: line.old, new: line.new, text: &line.text })
            }
        }
    }

    fn shown_lines(&self) -> impl Iterator<Item = (Spot, Numbered<'_>)> {
        self.rows.iter().filter_map(|row| row.spot()).filter_map(|spot| Some((spot, self.line_at(spot)?)))
    }

    fn covered(&self, selection: Selection<Spot>) -> Vec<(Spot, Numbered<'_>)> {
        self.shown_lines().filter(|(spot, line)| selection.lit(*spot, line.text).covers()).collect()
    }

    pub(super) fn selected_text(&self, selection: Selection<Spot>) -> String {
        let parts = selection.parts(self.shown_lines().map(|(spot, line)| (spot, line.text)));
        let cut = |(spot, range): (Spot, std::ops::Range<usize>)| {
            self.line_at(spot).and_then(|line| line.text.get(range)).unwrap_or_default()
        };
        parts.into_iter().map(cut).collect::<Vec<_>>().join("\n")
    }

    pub(super) fn selected_lines(&self, selection: Selection<Spot>) -> Option<(u32, u32)> {
        let covered = self.covered(selection);
        let mut new = covered.iter().filter_map(|(_, line)| line.new);
        let first = new.next();
        if let Some(first) = first {
            return Some((first, new.next_back().unwrap_or(first)));
        }
        let (spot, _) = covered.first()?;
        self.patch.hunks.get(spot.hunk).map(diff::changed)
    }

    pub(super) fn selection_label(&self, selection: Selection<Spot>) -> Option<String> {
        let covered = self.covered(selection);
        let number = |pick: fn(&Numbered<'_>) -> Option<u32>| {
            let mut numbers = covered.iter().filter_map(|(_, line)| pick(line));
            let first = numbers.next()?;
            Some((first, numbers.next_back().unwrap_or(first)))
        };
        match number(|line| line.new) {
            Some(lines) => Some(selection::lines_label(lines)),
            None => number(|line| line.old).map(|lines| format!("removed {}", selection::lines_label(lines))),
        }
    }
}

pub(super) fn copied_toast(label: &str, text: &str) -> String {
    if text.contains("    ") { format!("Copied {label}; {NO_OWN_TEXT}") } else { format!("Copied {label}") }
}

impl Selects for ChangesPanel {
    type Spot = Spot;

    fn selecting(&mut self) -> Option<&mut Selecting<Spot>> {
        Some(&mut self.open.selecting)
    }

    fn scroll_a_row(&mut self, edge: Edge) {
        self.diff_list.scroll_by(edge.a_row());
    }

    fn line_text(&self, spot: Spot) -> Option<&str> {
        self.open.data.as_ref()?.line_at(spot).map(|line| line.text)
    }

    fn selected_text(&self) -> Option<String> {
        Some(self.open.data.as_ref()?.selected_text(self.open.selecting.selection?))
    }
}

impl ChangesPanel {
    pub(crate) fn escape(&mut self, cx: &mut Context<Self>) -> bool {
        let gone = self.open.selecting.selection.take().is_some();
        if gone {
            cx.notify();
        }
        gone
    }

    pub(crate) fn copy_selection(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(selection) = self.open.selecting.selection else { return false };
        let Some(data) = self.open.data.clone() else { return false };
        let text = data.selected_text(selection);
        let label = data.selection_label(selection).unwrap_or_else(|| "the selection".to_string());
        toast(copied_toast(&label, &text), ToastKind::Info, cx);
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        true
    }

    fn ask_about_selection(&mut self, cx: &mut Context<Self>) {
        let Some(selection) = self.open.selecting.selection else { return };
        let Some(data) = self.open.data.clone() else { return };
        let Some(lines) = data.selected_lines(selection) else { return };
        let reference = selection::reference(&data.patch.file.path, lines);
        let workspace = data.selected.workspace;
        Store::get(cx).update(cx, |s, cx| s.ask_about(workspace, &reference, cx));
    }

    fn open_selection(&mut self, cx: &mut Context<Self>) {
        let Some(selection) = self.open.selecting.selection else { return };
        let Some(data) = self.open.data.clone() else { return };
        let Some((first, _)) = data.selected_lines(selection) else { return };
        Store::get(cx).read(cx).open_in_editor(data.selected.workspace, &data.patch.file.path, first);
    }

    pub(super) fn line_bar(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let selection = self.open.selecting.selection?;
        let data = self.open.data.as_ref()?;
        let label = data.selection_label(selection)?;
        let mut actions = Vec::new();
        if data.patch.file.status != FileStatus::Deleted {
            let open = selection::action("diff-open", "Open")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.open_selection(cx)));
            actions.push(open.into_any_element());
        }
        let ask = selection::action("diff-ask", "Ask agent")
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.ask_about_selection(cx)));
        let copy = selection::action("diff-copy", "Copy").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.copy_selection(cx);
        }));
        actions.extend([ask.into_any_element(), copy.into_any_element()]);
        Some(selection::action_bar(label, actions).into_any_element())
    }
}

#[cfg(test)]
mod tests;
