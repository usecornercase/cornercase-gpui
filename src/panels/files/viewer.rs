use std::path::PathBuf;
use std::sync::Arc;

use gpui::{AppContext as _, Context, ScrollStrategy};
use grep_regex::RegexMatcher;

use super::disk::{self, Content};
use super::{FilesPanel, Spans, Viewer, highlight, tree};

#[derive(Default)]
pub(super) struct OpenAt {
    pub(super) line: Option<usize>,
    pub(super) lit: Option<RegexMatcher>,
    pub(super) from_search: bool,
}

struct Reading {
    id: u64,
    path: PathBuf,
    stamp: Option<disk::Stamp>,
    language: Option<&'static str>,
}

fn text_lines(content: &Content) -> Option<Vec<String>> {
    match content {
        Content::Text(text) => Some(text.lines.clone()),
        _ => None,
    }
}

impl Viewer {
    fn new(path: PathBuf, rel: PathBuf, at: OpenAt) -> Self {
        Self {
            path,
            rel,
            content: None,
            spans: None,
            language: None,
            stamp: None,
            selected: at.line,
            scroll_to: Some(at.line.unwrap_or(0)),
            lit: at.lit,
            checking: true,
            from_search: at.from_search,
        }
    }

    fn set_content(&mut self, content: Content, reading: &Reading) {
        if let Content::Text(text) = &content
            && self.selected.is_some_and(|l| l >= text.lines.len())
        {
            self.selected = None;
        }
        self.content = Some(content);
        self.stamp = reading.stamp;
        self.language = reading.language;
    }
}

impl FilesPanel {
    pub(super) fn open_file(&mut self, rel: PathBuf, at: OpenAt, cx: &mut Context<Self>) {
        let Some(id) = self.current else { return };
        let Some(place) = self.places.get_mut(&id) else { return };
        let path = place.root.join(&rel);
        place.last = Some(rel.clone());
        let opened = tree::reveal(&mut place.expanded, &rel);
        place.viewer = Some(Viewer::new(path.clone(), rel, at));
        if !opened.is_empty() {
            self.list(id, opened, cx);
        }
        Self::read_file(id, path, true, cx);
        cx.notify();
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
        if let Some(content) = content {
            viewer.set_content(content, reading);
        }
        viewer.spans = spans;
        viewer.checking = false;
        if let (Some(line), Some(Content::Text(_))) = (viewer.scroll_to.take(), &viewer.content) {
            self.code_scroll.scroll_to_item(line.saturating_sub(3), ScrollStrategy::Top);
        }
        cx.notify();
    }

    fn read_file(id: u64, path: PathBuf, first: bool, cx: &mut Context<Self>) {
        let read_path = path.clone();
        let read = cx.background_spawn(async move { disk::read(&read_path) });
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
        let Some(viewer) = self.places.get_mut(&id).and_then(|p| p.viewer.as_mut()) else { return };
        if viewer.checking {
            return;
        }
        viewer.checking = true;
        let path = viewer.path.clone();
        let known = viewer.stamp;
        let stamp_path = path.clone();
        let stamp = cx.background_spawn(async move { disk::stamp(&stamp_path) });
        cx.spawn(async move |this, cx| {
            let unchanged = stamp.await == known;
            let _ = this.update(cx, |this, cx| this.checked(id, path, unchanged, cx));
        })
        .detach();
    }

    fn checked(&mut self, id: u64, path: PathBuf, unchanged: bool, cx: &mut Context<Self>) {
        if !unchanged {
            Self::read_file(id, path, false, cx);
            return;
        }
        if let Some(viewer) = self.places.get_mut(&id).and_then(|p| p.viewer.as_mut()) {
            viewer.checking = false;
        }
    }

    pub(super) fn close_file(&mut self, cx: &mut Context<Self>) {
        self.menu = None;
        let show_hidden = self.show_hidden;
        let Some(place) = self.place_mut() else { return };
        place.viewer = None;
        let last = place.last.clone();
        let rows = tree::rows(&place.listings, &place.expanded, show_hidden);
        if let Some(ix) = last.and_then(|last| rows.iter().position(|r| r.path == last)) {
            self.tree_scroll.scroll_to_item(ix, ScrollStrategy::Center);
        }
        cx.notify();
    }

    pub(super) fn select_line(&mut self, line: usize, cx: &mut Context<Self>) {
        if let Some(viewer) = self.place_mut().and_then(|p| p.viewer.as_mut()) {
            viewer.selected = if viewer.selected == Some(line) { None } else { Some(line) };
        }
        cx.notify();
    }
}
