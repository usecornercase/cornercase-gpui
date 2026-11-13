use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::{
    AnyView, App, AppContext as _, Context, ListAlignment, ListOffset, ListState, ScrollStrategy, WeakEntity, Window,
    px,
};

use super::disk::{self, Content};
use super::find::{FIELD_ROOM, Find};
use super::gutter::Margin;
use super::source::Source;
use super::style::small_f32;
use super::{FilesPanel, Spans, Viewer, highlight, tree};
use crate::model::Link;
use crate::panels::PanelKind;
use crate::platform::paths::{self, Style};
use crate::prefs::LongLines;
use crate::store::Store;
use crate::ui::selection::{CODE_ROW, Caret, Fit, Off, Selecting, Selection, Step};

const OVERDRAW: f32 = 400.;
const ROWS_ABOVE_A_LINE: usize = 3;
const ROWS_WITHOUT_A_FRAME: usize = 30;

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "a height in pixels, at least a row")]
fn rows_in(height: f32) -> usize {
    if height < CODE_ROW { ROWS_WITHOUT_A_FRAME } else { (height / CODE_ROW) as usize }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Reveal {
    spot: (usize, usize),
    waited: bool,
    tried_its_line: bool,
}

#[derive(Clone)]
struct Fetch {
    id: u64,
    path: PathBuf,
    rel: PathBuf,
    source: Source,
}

#[derive(Default)]
pub(super) struct OpenAt {
    pub(super) line: Option<usize>,
    pub(super) through: Option<usize>,
    pub(super) query: Option<String>,
    pub(super) from_search: bool,
    pub(super) from_flow: bool,
}

impl OpenAt {
    fn selection(&self) -> Option<Selection<usize>> {
        let anchor = self.line?;
        Some(Selection::Lines { anchor, head: self.through.unwrap_or(anchor) })
    }
}

struct Reading {
    id: u64,
    path: PathBuf,
    stamp: Option<disk::Stamp>,
    language: Option<&'static str>,
}

pub(super) fn top_for(lines: &[String], (line, at): (usize, usize), fit: Fit, mut above: usize) -> (usize, usize) {
    let inside = fit.row_at(at);
    if inside >= above {
        return (line, inside - above);
    }
    above -= inside;
    for ix in (0..line).rev() {
        let taken = fit.rows_of(lines.get(ix).map_or(0, String::len));
        if taken >= above {
            return (ix, taken - above);
        }
        above -= taken;
    }
    (0, 0)
}

fn text_lines(content: &Content) -> Option<Arc<Vec<String>>> {
    match content {
        Content::Text(text) => Some(text.lines.clone()),
        _ => None,
    }
}

impl Viewer {
    fn new(path: PathBuf, rel: PathBuf, at: &OpenAt) -> Self {
        Self {
            path,
            rel,
            content: None,
            spans: None,
            language: None,
            stamp: None,
            selecting: Selecting::new(at.selection()),
            list: ListState::new(0, ListAlignment::Top, px(OVERDRAW)),
            long_lines: LongLines::default(),
            margin: Margin::default(),
            reveal: None,
            scroll_to: Some(at.line.unwrap_or(0)),
            find: None,
            checking: true,
            from_search: at.from_search,
            from_flow: at.from_flow,
        }
    }

    fn set_content(&mut self, content: Content, reading: &Reading) {
        if let Content::Text(text) = &content {
            self.selecting.selection = self.selecting.selection.and_then(|s| s.fit(text.lines.len()));
        }
        self.content = Some(content);
        self.stamp = reading.stamp;
        self.language = reading.language;
        self.refit();
    }

    pub(super) fn line_count(&self) -> usize {
        match &self.content {
            Some(Content::Text(text)) => text.lines.len(),
            _ => 0,
        }
    }

    pub(super) fn fit(&self) -> Fit {
        Fit::of(self.long_lines == LongLines::Wrap, self.selecting.laid().body().size.width)
    }

    pub(super) fn room(&self) -> f32 {
        if self.find.is_some() { FIELD_ROOM } else { 0. }
    }

    fn show_near(&self, spot: (usize, usize), rows_above: usize) {
        let Some(Content::Text(text)) = &self.content else { return };
        let (item_ix, rows) = top_for(&text.lines, spot, self.fit(), rows_above);
        self.list.scroll_to(ListOffset { item_ix, offset_in_item: px(small_f32(rows) * CODE_ROW) });
    }

    pub(super) fn go_near(&mut self, spot: (usize, usize), rows_above: usize) {
        self.show_near(spot, rows_above);
        self.reveal = Some(Reveal { spot, waited: false, tried_its_line: false });
    }

    pub(super) fn bring_into_view(&mut self, spot: (usize, usize), off: Off) {
        match off {
            Off::InView => {}
            Off::By(distance) => self.list.scroll_by(distance),
            Off::Gone => {
                let rows = rows_in(f32::from(self.selecting.laid().body().size.height) - self.room());
                self.go_near(spot, rows / 2);
            }
        }
    }

    fn top_line_start(&self, count: usize) -> Caret<usize> {
        Caret { spot: self.list.logical_scroll_top().item_ix.min(count.saturating_sub(1)), at: 0 }
    }

    pub(super) fn step_caret(&mut self, step: Step, lines: &[String]) {
        let (fit, room) = (self.fit(), self.room());
        let top = self.selecting.laid().first_shown(room).unwrap_or_else(|| self.top_line_start(lines.len()));
        let Some(head) = self.selecting.step((step, top), lines, fit) else { return };
        let off = self.selecting.laid().past_edge(head.spot, head.at, room);
        self.bring_into_view((head.spot, head.at), off);
    }

    fn settle_reveal(&mut self, window: &Window) {
        let Some(reveal) = self.reveal else { return };
        if !reveal.waited {
            self.reveal = Some(Reveal { waited: true, ..reveal });
            window.request_animation_frame();
            return;
        }
        self.reveal = None;
        match self.selecting.laid().off(reveal.spot.0, reveal.spot.1, self.room()) {
            Off::InView => {}
            Off::By(distance) => self.list.scroll_by(distance),
            Off::Gone if reveal.tried_its_line => {}
            Off::Gone => {
                self.show_near((reveal.spot.0, 0), 0);
                self.reveal = Some(Reveal { waited: false, tried_its_line: true, ..reveal });
                window.request_animation_frame();
            }
        }
    }

    pub(super) fn remeasure(&self, line: usize) {
        let item = line.min(self.line_count().saturating_sub(1));
        self.list.splice(item..item + 1, 1);
    }

    pub(super) fn refit(&self) {
        let top = self.list.logical_scroll_top();
        self.list.reset(self.line_count());
        self.list.scroll_to(top);
    }
}

pub(crate) fn open_link(files: &AnyView, link: &Link, cx: &mut App) {
    if let Ok(panel) = files.clone().downcast::<FilesPanel>() {
        show_link(&panel.downgrade(), link, None, cx);
    }
}

pub(super) fn show_link(panel: &WeakEntity<FilesPanel>, link: &Link, at: Option<usize>, cx: &mut App) {
    let _ = panel.update(cx, |panel, cx| panel.open_link(link, at, cx));
    dock_files(cx);
}

pub(super) fn dock_files(cx: &mut App) {
    Store::get(cx).update(cx, |s, cx| {
        s.close_dialog(cx);
        s.close_sheet(cx);
        s.open(PanelKind::Files, cx);
    });
}

fn link_path(link: &Link, shown: Option<u64>) -> PathBuf {
    match &link.relative {
        Some(rel) if link.inside && link.workspace == shown => Style::of_path(&link.path).relative(rel),
        _ => PathBuf::from(&link.path),
    }
}

impl FilesPanel {
    pub(super) fn settle_reveal(&mut self, window: &Window) {
        if let Some(viewer) = self.place_mut().and_then(|place| place.viewer.as_mut()) {
            viewer.settle_reveal(window);
        }
    }

    fn open_link(&mut self, link: &Link, at: Option<usize>, cx: &mut Context<Self>) {
        let from_search = self.place().is_some_and(|p| !p.query.trim().is_empty());
        let through = at.is_none().then(|| link.last_line()).flatten();
        let target = OpenAt { line: at.or_else(|| link.first_line()), through, from_search, ..OpenAt::default() };
        self.menu = None;
        cx.notify();
        self.open_file(link_path(link, self.current), target, cx);
    }

    pub(super) fn open_file(&mut self, rel: PathBuf, mut at: OpenAt, cx: &mut Context<Self>) {
        if self.refuses_asking(cx).is_some() {
            return;
        }
        let Some(id) = self.current else { return };
        let Some(place) = self.places.get_mut(&id) else { return };
        let path = place.style.join(&place.root, &rel);
        place.last = Some(rel.clone());
        let opened =
            if paths::rooted(&rel.to_string_lossy()) { Vec::new() } else { tree::reveal(&mut place.expanded, &rel) };
        let fetch = Fetch { id, path: path.clone(), rel: rel.clone(), source: place.source.clone() };
        let find = at.query.take().map(|query| Find::new(&query, cx));
        place.viewer = Some(Viewer { find, ..Viewer::new(path, rel, &at) });
        if !opened.is_empty() {
            self.list(id, opened, cx);
        }
        Self::read_file(fetch, true, cx);
        cx.notify();
    }

    pub(super) fn reveal(&mut self, id: u64, root: &Path, rel: PathBuf, cx: &mut Context<Self>) {
        let Some(place) = self.place().filter(|p| self.current == Some(id) && p.root == root) else { return };
        let from_search = !place.query.trim().is_empty();
        self.open_file(rel, OpenAt { from_search, ..OpenAt::default() }, cx);
    }

    fn apply_read(
        &mut self,
        reading: &Reading,
        content: Option<Content>,
        spans: Option<Spans>,
        cx: &mut Context<Self>,
    ) {
        self.machine_read_answered(reading.id, content.as_ref(), cx);
        let Some(viewer) = self.places.get_mut(&reading.id).and_then(|p| p.viewer.as_mut()) else { return };
        if viewer.path != reading.path {
            return;
        }
        if matches!(content, Some(Content::Lost(_))) {
            viewer.checking = false;
            cx.notify();
            return;
        }
        let fresh = content.is_some();
        if let Some(content) = content {
            viewer.set_content(content, reading);
        }
        viewer.spans = spans;
        viewer.checking = false;
        if let Some(line) = viewer.scroll_to.take() {
            viewer.go_near((line, 0), ROWS_ABOVE_A_LINE);
        }
        if fresh {
            self.find_again(reading.id, cx);
            self.follow_changes(reading.id, true, cx);
        }
        cx.notify();
    }

    fn read_file(fetch: Fetch, first: bool, cx: &mut Context<Self>) {
        let Fetch { id, path, rel, source } = fetch;
        let read = cx.background_spawn(async move { source.read(&rel) });
        cx.spawn(async move |this, cx| {
            let (content, stamp) = read.await;
            let lines = text_lines(&content);
            let language =
                lines.as_ref().and_then(|l| highlight::language(&path, l.first().map_or("", String::as_str)));
            let reading = Reading { id, path, stamp, language };
            let Some(lines) = lines else {
                let _ = this.update(cx, |this, cx| this.apply_read(&reading, Some(content), None, cx));
                return;
            };
            let mut content = Some(content);
            if first {
                let _ = this.update(cx, |this, cx| this.apply_read(&reading, content.take(), None, cx));
            }
            let spans_path = reading.path.clone();
            let spans = cx.background_spawn(async move { highlight::highlight(&spans_path, &lines).map(Arc::new) });
            let spans = spans.await;
            let _ = this.update(cx, |this, cx| this.apply_read(&reading, content, spans, cx));
        })
        .detach();
    }

    pub(super) fn check_file(&mut self, cx: &mut Context<Self>) {
        if !self.may_poll(cx) {
            return;
        }
        let Some(id) = self.current else { return };
        let Some(place) = self.places.get_mut(&id) else { return };
        let source = place.source.clone();
        let Some(viewer) = place.viewer.as_mut() else { return };
        if viewer.checking {
            return;
        }
        viewer.checking = true;
        let fetch = Fetch { id, path: viewer.path.clone(), rel: viewer.rel.clone(), source };
        let known = viewer.content.is_some().then_some(viewer.stamp);
        let asked = fetch.clone();
        let stamp = cx.background_spawn(async move { asked.source.stamp(&asked.rel) });
        cx.spawn(async move |this, cx| {
            let stamp = stamp.await;
            let unchanged = known.is_some_and(|known| known == stamp);
            let _ = this.update(cx, |this, cx| this.checked(fetch, unchanged, cx));
        })
        .detach();
    }

    fn checked(&mut self, fetch: Fetch, unchanged: bool, cx: &mut Context<Self>) {
        let id = fetch.id;
        if !unchanged {
            Self::read_file(fetch, false, cx);
            return;
        }
        if let Some(viewer) = self.places.get_mut(&id).and_then(|p| p.viewer.as_mut()) {
            viewer.checking = false;
        }
    }

    pub(super) fn take_run_file(&mut self, cx: &mut Context<Self>) {
        let store = self.store.clone();
        if let Some(path) = store.update(cx, |s, _| s.run_file.take()) {
            self.open_file(PathBuf::from(path), OpenAt { from_flow: true, ..OpenAt::default() }, cx);
        }
    }

    pub(super) fn forget_flow(&mut self) {
        let viewers = self.places.values_mut().filter_map(|place| place.viewer.as_mut());
        viewers.for_each(|viewer| viewer.from_flow = false);
    }

    fn back_to_flow(&self, cx: &mut Context<Self>) {
        self.store.update(cx, |s, cx| {
            if PanelKind::Flow.shows(s.session.as_ref()) {
                s.open(PanelKind::Flow, cx);
            }
        });
    }

    pub(super) fn close_file(&mut self, cx: &mut Context<Self>) {
        self.menu = None;
        let show_hidden = self.show_hidden;
        let Some(place) = self.place_mut() else { return };
        let from_flow = place.viewer.take().is_some_and(|viewer| viewer.from_flow);
        let last = place.last.clone();
        let rows = tree::rows(&place.listings, &place.expanded, show_hidden);
        if let Some(ix) = last.and_then(|last| rows.iter().position(|r| r.path == last)) {
            self.tree_scroll.scroll_to_item(ix, ScrollStrategy::Center);
        }
        if from_flow {
            self.back_to_flow(cx);
        }
        self.store.update(cx, Store::leave_file_full_screen);
        cx.notify();
    }
}

#[cfg(test)]
mod tests;
