use gpui::{
    AnyElement, ClickEvent, ClipboardItem, Context, IntoElement, Pixels, Point, StatefulInteractiveElement as _,
};

use crate::ui::selection::{self, Edge, Selecting, Selection, Selects};

use super::disk::{Content, Text};
use super::{FilesPanel, Target, Viewer};

pub(super) fn one_based((first, last): (usize, usize)) -> (usize, usize) {
    (first + 1, last + 1)
}

pub(super) fn selected_text(text: &Text, selection: Selection<usize>) -> String {
    let (first, last) = selection.ends();
    let lines = (first..=last).filter_map(|ix| text.lines.get(ix).map(|line| (ix, line.as_str())));
    let parts = selection.parts(lines);
    parts.into_iter().map(|(ix, range)| text.own(ix, range)).collect::<Vec<_>>().join("\n")
}

impl Viewer {
    fn text(&self) -> Option<&Text> {
        match &self.content {
            Some(Content::Text(text)) => Some(text),
            _ => None,
        }
    }
}

impl Selects for FilesPanel {
    type Spot = usize;

    fn selecting(&mut self) -> Option<&mut Selecting<usize>> {
        self.place_mut()?.viewer.as_mut().map(|viewer| &mut viewer.selecting)
    }

    fn scroll_a_row(&mut self, edge: Edge) {
        if let Some(viewer) = self.shown_viewer() {
            viewer.list.scroll_by(edge.a_row());
        }
    }

    fn line_text(&self, spot: usize) -> Option<&str> {
        self.shown_viewer()?.text()?.lines.get(spot).map(String::as_str)
    }

    fn selected_text(&self) -> Option<String> {
        let viewer = self.shown_viewer()?;
        Some(selected_text(viewer.text()?, viewer.selecting.selection?))
    }
}

impl FilesPanel {
    fn shown_viewer(&self) -> Option<&Viewer> {
        self.place()?.viewer.as_ref()
    }

    fn covered(&self) -> Option<(usize, usize)> {
        Some(self.shown_viewer()?.selecting.selection?.covered())
    }

    pub(crate) fn copy_selection(&mut self, cx: &mut Context<Self>) -> bool {
        let (Some(text), Some(lines)) = (self.selected_text(), self.covered()) else { return false };
        let what = selection::lines_label(one_based(lines));
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.store.update(cx, |s, cx| s.toast(format!("Copied {what}"), crate::store::ToastKind::Info, cx));
        true
    }

    fn ask_about_selection(&mut self, cx: &mut Context<Self>) {
        let (Some(id), Some(viewer), Some(lines)) = (self.current, self.shown_viewer(), self.covered()) else { return };
        let reference = selection::reference(&self.style().shown(&viewer.rel), one_based(lines));
        self.store.update(cx, |s, cx| s.ask_about(id, &reference, cx));
    }

    fn open_selection(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        let (Some(id), Some(viewer), Some((first, _))) = (self.current, self.shown_viewer(), self.covered()) else {
            return;
        };
        if self.remote {
            let path = self.style().shown(&viewer.rel);
            self.store.read(cx).open_in_editor(id, &path, first + 1);
            return;
        }
        let target = Target { rel: viewer.rel.clone(), dir: false, line: Some(first) };
        self.show_menu(at, target, cx);
    }

    pub(super) fn line_bar(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.shown_viewer()?.text()?;
        let label = selection::lines_label(one_based(self.covered()?));
        let actions = vec![
            selection::action("viewer-open", "Open")
                .on_click(cx.listener(|this, e: &ClickEvent, _, cx| this.open_selection(e.position(), cx)))
                .into_any_element(),
            selection::action("viewer-ask", "Ask agent")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.ask_about_selection(cx)))
                .into_any_element(),
            selection::action("viewer-copy", "Copy")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.copy_selection(cx);
                }))
                .into_any_element(),
        ];
        Some(selection::action_bar(label, actions).into_any_element())
    }
}

#[cfg(test)]
mod tests;
