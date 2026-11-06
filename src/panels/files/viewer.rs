use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::{AnyView, App, AppContext as _, Context, ScrollStrategy, WeakEntity};

use super::disk::{self, Content};
use super::find::Find;
use super::source::Source;
use super::{FilesPanel, Spans, Viewer, highlight, tree};
use crate::model::Link;
use crate::panels::PanelKind;
use crate::platform::paths::{self, Style};
use crate::store::Store;
use crate::ui::selection::{Selecting, Selection};

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
    fn open_link(&mut self, link: &Link, at: Option<usize>, cx: &mut Context<Self>) {
        let from_search = self.place().is_some_and(|p| !p.query.trim().is_empty());
        let through = at.is_none().then(|| link.last_line()).flatten();
        let target = OpenAt { line: at.or_else(|| link.first_line()), through, from_search, ..OpenAt::default() };
        self.menu = None;
        self.open_file(link_path(link, self.current), target, cx);
    }

    pub(super) fn open_file(&mut self, rel: PathBuf, mut at: OpenAt, cx: &mut Context<Self>) {
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
        let Some(viewer) = self.places.get_mut(&reading.id).and_then(|p| p.viewer.as_mut()) else { return };
        if viewer.path != reading.path {
            return;
        }
        let fresh = content.is_some();
        if let Some(content) = content {
            viewer.set_content(content, reading);
        }
        viewer.spans = spans;
        viewer.checking = false;
        if let (Some(line), Some(Content::Text(_))) = (viewer.scroll_to.take(), &viewer.content) {
            self.code_scroll.scroll_to_item(line.saturating_sub(3), ScrollStrategy::Top);
        }
        if fresh {
            self.find_again(reading.id, cx);
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
        let Some(id) = self.current else { return };
        let Some(place) = self.places.get_mut(&id) else { return };
        let source = place.source.clone();
        let Some(viewer) = place.viewer.as_mut() else { return };
        if viewer.checking {
            return;
        }
        viewer.checking = true;
        let fetch = Fetch { id, path: viewer.path.clone(), rel: viewer.rel.clone(), source };
        let known = viewer.stamp;
        let asked = fetch.clone();
        let stamp = cx.background_spawn(async move { asked.source.stamp(&asked.rel) });
        cx.spawn(async move |this, cx| {
            let unchanged = stamp.await == known;
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
        cx.notify();
    }
}

#[cfg(test)]
mod tests;
