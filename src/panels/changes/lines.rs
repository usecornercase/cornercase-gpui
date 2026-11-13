use std::ops::Range;

use gpui::{
    AnyElement, ClickEvent, ClipboardItem, Context, IntoElement, ListOffset, Pixels, StatefulInteractiveElement as _,
    px,
};

use crate::escape::{self, Top};
use crate::store::{Store, ToastKind};
use crate::ui::selection::{self, Caret, Edge, Fit, Off, Selecting, Selection, Selects, ShownLines, Step};

use super::diff::{self, Part, Spot};
use super::pieces::toast;
use super::wire::FileStatus;
use super::{ChangesPanel, DiffData, Whose};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Numbered<'a> {
    old: Option<u32>,
    new: Option<u32>,
    text: &'a str,
    raw: Option<&'a str>,
}

impl<'a> Numbered<'a> {
    fn own(&self, shown: Range<usize>) -> &'a str {
        match selection::own_if_drawn(self.raw, self.text) {
            Some(raw) => selection::own_part(raw, shown),
            None => self.text.get(shown).unwrap_or_default(),
        }
    }
}

impl DiffData {
    pub(super) fn line_at(&self, spot: Spot) -> Option<Numbered<'_>> {
        match spot.part {
            Part::Gap => {
                let line = self.gaps.get(&spot.hunk)?.0.lines.get(spot.line)?;
                Some(Numbered { old: Some(line.old), new: Some(line.new), text: &line.text, raw: line.raw.as_deref() })
            }
            Part::Patch => {
                let line = self.patch.hunks.get(spot.hunk)?.lines.get(spot.line)?;
                Some(Numbered { old: line.old, new: line.new, text: &line.text, raw: line.raw.as_deref() })
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
        let shown = self.shown_lines().map(|(spot, line)| (spot, line.text));
        selection.text(shown, |spot, range| self.line_at(spot).map(|line| line.own(range)).unwrap_or_default())
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

    pub(super) fn copied(&self, selection: Selection<Spot>) -> Option<(String, String)> {
        if selection.is_empty() {
            return None;
        }
        let label = self.selection_label(selection).unwrap_or_else(|| "the selection".to_string());
        Some((self.selected_text(selection), label))
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

impl ShownLines<Spot> for DiffData {
    fn text(&self, spot: Spot) -> Option<&str> {
        self.line_at(spot).map(|line| line.text)
    }

    fn after(&self, spot: Spot) -> Option<Spot> {
        diff::line_after(&self.rows, spot)
    }

    fn before(&self, spot: Spot) -> Option<Spot> {
        diff::line_before(&self.rows, spot)
    }
}

pub(super) fn top_under(ix: usize, before: Option<Pixels>) -> (usize, Pixels) {
    match (ix.checked_sub(1), before) {
        (Some(above), Some(height)) => (above, (height - px(selection::CODE_ROW)).max(px(0.))),
        _ => (ix, px(0.)),
    }
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
        ShownLines::text(&**self.open.data.as_ref()?, spot)
    }

    fn selected_text(&self) -> Option<String> {
        Some(self.open.data.as_ref()?.selected_text(self.open.selecting.selection?))
    }
}

impl ChangesPanel {
    pub(crate) fn tops(&self) -> Vec<Top> {
        escape::tops_of(&[(Top::Selection, self.open.selecting.shows())])
    }

    pub(crate) fn close_top(&mut self, top: Top, cx: &mut Context<Self>) {
        if top == Top::Selection {
            self.open.selecting.clear();
            cx.notify();
        }
    }

    pub(crate) fn copy_selection(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(selection) = self.open.selecting.selection else { return false };
        let Some(data) = self.open.data.clone() else { return false };
        let Some((text, label)) = data.copied(selection) else { return false };
        toast(format!("Copied {label}"), ToastKind::Info, cx);
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        true
    }

    pub(super) fn fit(&self) -> Fit {
        Fit::of(self.wraps, self.open.selecting.laid().body().size.width)
    }

    fn first_shown(&self, data: &DiffData) -> Option<Caret<Spot>> {
        if let Some(shown) = self.open.selecting.laid().first_shown(0.) {
            return Some(shown);
        }
        let spot = diff::first_line_from(&data.rows, self.diff_list.logical_scroll_top().item_ix)?;
        Some(Caret { spot, at: 0 })
    }

    pub(crate) fn extend_selection(&mut self, step: Step, cx: &mut Context<Self>) -> bool {
        let Some(data) = self.open.data.clone() else { return false };
        let Some(top) = self.first_shown(&data) else { return false };
        let fit = self.fit();
        if let Some(head) = self.open.selecting.step((step, top), &*data, fit) {
            self.follow_head(&data, head);
        }
        selection::to_primary(self, cx);
        cx.notify();
        true
    }

    fn follow_head(&self, data: &DiffData, head: Caret<Spot>) {
        let laid = self.open.selecting.laid();
        let row = diff::row_of(&data.rows, head.spot);
        let mut off = laid.past_edge(head.spot, head.at, 0.);
        if off == Off::Gone {
            let bounds = row.and_then(|ix| self.diff_list.bounds_for_item(ix));
            off = bounds.map_or(Off::Gone, |bounds| laid.past_edge_of(bounds, 0.));
        }
        match (off, row) {
            (Off::By(distance), _) => self.diff_list.scroll_by(distance),
            (Off::Gone, Some(ix)) => self.line_to_top(ix),
            (Off::InView | Off::Gone, _) => {}
        }
    }

    fn line_to_top(&self, ix: usize) {
        let mut above = None;
        if let Some(before) = ix.checked_sub(1) {
            self.diff_list.scroll_to(ListOffset { item_ix: before, offset_in_item: px(0.) });
            above = self.diff_list.bounds_for_item(before).map(|row| row.size.height);
        }
        let (item_ix, offset_in_item) = top_under(ix, above);
        self.diff_list.scroll_to(ListOffset { item_ix, offset_in_item });
    }

    fn ask_about_selection(&mut self, cx: &mut Context<Self>) {
        let Some(selection) = self.open.selecting.selection else { return };
        let Some(data) = self.open.data.clone() else { return };
        let Some(lines) = data.selected_lines(selection) else { return };
        let reference = selection::reference(&data.patch.file.path, lines);
        let Whose::Here(workspace) = data.selected.workspace else { return };
        Store::get(cx).update(cx, |s, cx| s.ask_about(workspace, &reference, cx));
    }

    fn open_selection(&mut self, cx: &mut Context<Self>) {
        let Some(selection) = self.open.selecting.selection else { return };
        let Some(data) = self.open.data.clone() else { return };
        let Some((first, _)) = data.selected_lines(selection) else { return };
        let Whose::Here(workspace) = data.selected.workspace else { return };
        Store::get(cx).read(cx).open_in_editor(workspace, &data.patch.file.path, first);
    }

    pub(super) fn line_bar(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let selection = self.open.selecting.selection?;
        let data = self.open.data.as_ref()?;
        let label = data.selection_label(selection)?;
        let mut actions = Vec::new();
        if matches!(data.selected.workspace, Whose::Here(_)) {
            actions = Self::line_acts(data, cx);
        }
        let copy = selection::action("diff-copy", "Copy").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.copy_selection(cx);
        }));
        actions.push(copy.into_any_element());
        Some(selection::action_bar(label, actions).into_any_element())
    }

    fn line_acts(data: &DiffData, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut actions = Vec::new();
        if data.patch.file.status != FileStatus::Deleted {
            let open = selection::action("diff-open", "Open")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.open_selection(cx)));
            actions.push(open.into_any_element());
        }
        let ask = selection::action("diff-ask", "Ask agent")
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.ask_about_selection(cx)));
        actions.push(ask.into_any_element());
        actions
    }
}

#[cfg(test)]
mod tests;
