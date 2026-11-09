use std::path::PathBuf;

use gpui::{
    AnyElement, AppContext as _, ClickEvent, Context, Div, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, Pixels, Rgba, StatefulInteractiveElement as _, Styled as _, div, px,
};

use crate::notify::tip;
use crate::theme::{ansi, colours};
use crate::ui;
use crate::ui::selection::{self, Fit};

use super::gutter::{Block, Follow, Gutter, Mark};
use super::served::Listed;
use super::source::Source;
use super::style::{code_frame, number_cell};
use super::{FilesPanel, tree};

const OLD_LINES_SHOWN: usize = 200;
const REMOVED_TINT: f32 = 0.11;

struct Asked {
    id: u64,
    path: PathBuf,
    rel: PathBuf,
    generation: u64,
}

fn line_bar(colour: impl Into<gpui::Fill>) -> Div {
    div().w(px(3.)).h_full().rounded_full().bg(colour)
}

fn edge_bar() -> Div {
    div().absolute().left_0().right_0().h(px(3.)).rounded_full().bg(colours::failed())
}

fn mark_bar(mark: Mark) -> Div {
    match mark {
        Mark::Added => line_bar(colours::done()),
        Mark::Changed => line_bar(ansi(4)),
        Mark::Removed => edge_bar().bottom_0(),
        Mark::RemovedAbove => edge_bar().top_0(),
    }
}

pub(super) fn mark_cell(line: usize, block: &Block, cx: &mut Context<FilesPanel>) -> AnyElement {
    let cell = div()
        .id(("files-mark", line))
        .absolute()
        .top_0()
        .bottom_0()
        .right(px(1.))
        .w(px(8.))
        .flex()
        .justify_center()
        .child(mark_bar(block.mark));
    if block.old.is_empty() {
        return cell.into_any_element();
    }
    let before = block.before;
    cell.cursor_pointer()
        .tooltip(tip("Show or hide the old lines"))
        .on_mouse_down(MouseButton::Left, selection::keep_selection)
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_block(before, cx)))
        .into_any_element()
}

fn old_row(line: &str, fit: Fit, width: Pixels) -> Div {
    fit.row(code_frame(false))
        .bg(Rgba { a: REMOVED_TINT, ..colours::failed() })
        .child(number_cell(width).text_color(colours::failed()).child("−"))
        .child(div().flex_1().min_w_0().pr(px(8.)).child(selection::plain_text(line, fit)))
}

pub(super) fn old_lines(block: &Block, fit: Fit, width: Pixels) -> impl IntoElement + use<> {
    let more = block.old.len().saturating_sub(OLD_LINES_SHOWN);
    let note = (more > 0).then(|| {
        fit.row(code_frame(false))
            .child(number_cell(width))
            .child(ui::dim(format!("{more} more old lines: the Changes panel shows them all")))
    });
    div()
        .w_full()
        .flex()
        .flex_col()
        .children(block.old.iter().take(OLD_LINES_SHOWN).map(|line| old_row(line, fit, width)))
        .children(note)
}

impl FilesPanel {
    pub(super) fn read_marks(&mut self, id: u64, folders: Vec<PathBuf>, cx: &mut Context<Self>) {
        let Some(place) = self.places.get_mut(&id).filter(|place| !place.marking && !place.source.lists_marks()) else {
            return;
        };
        place.marking = true;
        let source = place.source.clone();
        let job = cx.background_spawn(async move { source.marks(folders) });
        cx.spawn(async move |this, cx| {
            let listed = job.await;
            let _ = this.update(cx, |this, cx| this.apply_marks(id, &listed, cx));
        })
        .detach();
    }

    fn apply_marks(&mut self, id: u64, listed: &Listed, cx: &mut Context<Self>) {
        let Some(place) = self.places.get_mut(&id) else { return };
        place.marking = false;
        let answered = listed.iter().filter_map(|(rel, listing)| Some((rel, listing.as_ref().ok()?)));
        let mut changed = false;
        for (rel, listing) in answered {
            changed |= tree::keep_marks(&mut place.marks, rel, listing);
        }
        if changed {
            cx.notify();
        }
        self.follow_changes(id, false, cx);
    }

    pub(super) fn follow_changes(&mut self, id: u64, again: bool, cx: &mut Context<Self>) {
        if !self.may_poll(cx) {
            return;
        }
        let Some(place) = self.places.get_mut(&id) else { return };
        let source = place.source.clone();
        let Some(viewer) = place.viewer.as_mut() else { return };
        let status = place.marks.get(&viewer.rel).copied();
        match viewer.margin.follow(status, again) {
            Follow::Nothing => {}
            Follow::Cleared => {
                viewer.refit();
                cx.notify();
            }
            Follow::Read(generation) => {
                let asked = Asked { id, path: viewer.path.clone(), rel: viewer.rel.clone(), generation };
                Self::read_gutter(asked, source, cx);
            }
        }
    }

    fn read_gutter(asked: Asked, source: Source, cx: &mut Context<Self>) {
        let rel = asked.rel.clone();
        let job = cx.background_spawn(async move { source.gutter(&rel) });
        cx.spawn(async move |this, cx| {
            let gutter = job.await;
            let _ = this.update(cx, |this, cx| this.apply_gutter(&asked, gutter, cx));
        })
        .detach();
    }

    fn apply_gutter(&mut self, asked: &Asked, gutter: Option<Gutter>, cx: &mut Context<Self>) {
        let Some(place) = self.places.get_mut(&asked.id) else { return };
        if gutter.is_none() && place.source.is_there() {
            return;
        }
        let Some(viewer) = place.viewer.as_mut().filter(|viewer| viewer.path == asked.path) else { return };
        let unfolded = !viewer.margin.unfolded.is_empty();
        if viewer.margin.answered(asked.generation, gutter) {
            if unfolded {
                viewer.refit();
            }
            cx.notify();
        }
    }

    fn toggle_block(&mut self, before: usize, cx: &mut Context<Self>) {
        let Some(viewer) = self.place_mut().and_then(|place| place.viewer.as_mut()) else { return };
        viewer.margin.toggle(before);
        viewer.remeasure(before);
        cx.notify();
    }
}
