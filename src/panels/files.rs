//! The workspace's files: a tree, a viewer and a search. Read from disk, as #345 says a GUI should
//! (cornercase's `design/files-panel.md` describes the TUI's panel, which this one follows).
//!
//! The panel follows the shown workspace and keeps, per workspace and in memory, the open folders,
//! the open file and the query. Listings, reads, highlighting and searches run off the main
//! thread; while the panel shows, the open folders are listed again every two seconds and the open
//! file is checked every second, so what an agent writes appears.

mod disk;
mod highlight;
mod open;
mod search;
mod tree;

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, AnyView, AppContext as _, ClickEvent, ClipboardItem, Context, Entity, Focusable as _, FontStyle,
    FontWeight, HighlightStyle, Hsla, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent, ObjectFit,
    ParentElement as _, Pixels, Point, Render, ScrollStrategy, SharedString, StatefulInteractiveElement as _,
    Styled as _, StyledImage as _, StyledText, Subscription, Task, UniformListScrollHandle, Window, anchored, deferred,
    div, img, prelude::FluentBuilder as _, px, uniform_list,
};
use grep_matcher::Matcher as _;
use grep_regex::RegexMatcher;

use crate::theme::FONT_FAMILY;
use crate::panels::PanelKind;
use crate::store::{Store, ToastKind};
use crate::theme::{ANSI, colours, hsla};
use crate::ui::{self, TextField, TextFieldEvent};

use disk::Content;
use highlight::Kind;
use open::{Launch, Opener};

/// How often the open file is checked for changes while the panel shows.
const CHECK_EVERY: Duration = Duration::from_secs(1);
/// The open folders are listed again every this many checks.
const LIST_EVERY: u32 = 2;
/// An index of the workspace's paths younger than this is reused by a new query.
const INDEX_FRESH: Duration = Duration::from_secs(10);
const CODE_SIZE: f32 = 12.;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Names,
    Text,
}

/// What the panel keeps per workspace.
struct Place {
    root: PathBuf,
    listings: HashMap<PathBuf, tree::Listing>,
    /// Why the root could not be listed.
    error: Option<String>,
    expanded: HashSet<PathBuf>,
    viewer: Option<Viewer>,
    /// The last file opened, marked in the tree.
    last: Option<PathBuf>,
    query: String,
    listing: bool,
}

impl Place {
    fn new(root: PathBuf) -> Self {
        Self {
            root,
            listings: HashMap::new(),
            error: None,
            expanded: HashSet::new(),
            viewer: None,
            last: None,
            query: String::new(),
            listing: false,
        }
    }
}

struct Viewer {
    /// Absolute.
    path: PathBuf,
    /// Relative to the workspace's folder.
    rel: PathBuf,
    /// `None` while the first read runs.
    content: Option<Content>,
    spans: Option<Arc<Vec<Vec<highlight::Span>>>>,
    language: Option<&'static str>,
    stamp: Option<disk::Stamp>,
    /// The selected line, 0-based.
    selected: Option<usize>,
    /// Scroll to this line once the text is there.
    scroll_to: Option<usize>,
    /// The text query it was opened from, lit in the lines.
    lit: Option<RegexMatcher>,
    checking: bool,
    /// Back goes to the search results rather than the tree.
    from_search: bool,
}

/// What a search answered.
enum Results {
    None,
    Names(Vec<search::NameHit>),
    Text(search::TextResults),
}

/// A row of the text results: a file, or one of its matched lines.
#[derive(Clone, Copy)]
enum TextRow {
    File(usize),
    Line(usize, usize),
}

struct Search {
    mode: Mode,
    /// The query the results answer.
    answered: String,
    results: Results,
    rows: Vec<TextRow>,
    running: Option<Arc<AtomicBool>>,
    generation: u64,
    /// Enter pressed before the answer came: open its first result when it does.
    enter: bool,
    index: Option<(PathBuf, Arc<Vec<String>>, Instant)>,
}

/// A right-click menu on a row, or the viewer's "Open in…".
struct Menu {
    at: Point<Pixels>,
    path: PathBuf,
    rel: PathBuf,
    dir: bool,
    line: Option<usize>,
}

pub(crate) struct FilesPanel {
    store: Entity<Store>,
    /// The shown workspace.
    current: Option<u64>,
    places: HashMap<u64, Place>,
    query: Entity<TextField>,
    search: Search,
    show_hidden: bool,
    rows: Vec<tree::Row>,
    openers: Arc<Vec<(Opener, Launch)>>,
    menu: Option<Menu>,
    tree_scroll: UniformListScrollHandle,
    code_scroll: UniformListScrollHandle,
    results_scroll: UniformListScrollHandle,
    _subscriptions: Vec<Subscription>,
    _tick: Task<()>,
}

impl FilesPanel {
    pub(crate) fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = Store::get(cx);
        let query = cx.new(|cx| TextField::new("Search files…", cx));
        let subscriptions = vec![
            cx.observe(&store, |this, _, cx| this.follow_workspace(cx)),
            cx.subscribe(&query, |this, field, event: &TextFieldEvent, cx| match event {
                TextFieldEvent::Changed => {
                    let text = field.read(cx).text();
                    this.query_changed(text, cx);
                }
                TextFieldEvent::Submit => this.enter(cx),
                TextFieldEvent::Cancel => field.update(cx, TextField::clear),
            }),
        ];
        let tick = cx.spawn(async move |this, cx| {
            let mut n = 0u32;
            loop {
                cx.background_executor().timer(CHECK_EVERY).await;
                n = n.wrapping_add(1);
                if this.update(cx, |this, cx| this.tick(n.is_multiple_of(LIST_EVERY), cx)).is_err() {
                    break;
                }
            }
        });
        let openers = cx.background_spawn(async { open::detect() });
        cx.spawn(async move |this, cx| {
            let openers = openers.await;
            let _ = this.update(cx, |this, _| this.openers = Arc::new(openers));
        })
        .detach();
        let mut panel = Self {
            store,
            current: None,
            places: HashMap::new(),
            query,
            search: Search {
                mode: Mode::Names,
                answered: String::new(),
                results: Results::None,
                rows: Vec::new(),
                running: None,
                generation: 0,
                enter: false,
                index: None,
            },
            show_hidden: true,
            rows: Vec::new(),
            openers: Arc::new(Vec::new()),
            menu: None,
            tree_scroll: UniformListScrollHandle::new(),
            code_scroll: UniformListScrollHandle::new(),
            results_scroll: UniformListScrollHandle::new(),
            _subscriptions: subscriptions,
            _tick: tick,
        };
        panel.follow_workspace(cx);
        panel
    }

    // -----------------------------------------------------------------------------------------
    // Following the shown workspace.

    /// Switches to the shown workspace when it changed (or its folder did).
    fn follow_workspace(&mut self, cx: &mut Context<Self>) {
        let shown = self.store.read(cx).session.as_ref().and_then(|session| {
            let (_, ws) = session.workspace(session.shown().workspace?)?;
            Some((ws.id, PathBuf::from(&ws.path)))
        });
        let Some((id, root)) = shown else {
            if self.current.take().is_some() {
                cx.notify();
            }
            return;
        };
        if self.current == Some(id) && self.places.get(&id).is_some_and(|p| p.root == root) {
            return;
        }
        // Keep the query of the workspace we leave, and bring back the one we return to.
        let old_query = self.query.read(cx).text();
        if let Some(place) = self.current.and_then(|id| self.places.get_mut(&id)) {
            place.query = old_query.clone();
        }
        if self.places.get(&id).is_some_and(|p| p.root != root) {
            self.places.remove(&id);
        }
        self.current = Some(id);
        self.menu = None;
        let place = self.places.entry(id).or_insert_with(|| Place::new(root));
        let query = place.query.clone();
        self.search.index = None;
        self.search.results = Results::None;
        self.search.rows.clear();
        self.search.answered.clear();
        if query != old_query {
            self.query.update(cx, |f, cx| f.set_text(query, cx));
        } else if !query.trim().is_empty() {
            self.run_search(cx);
        }
        self.relist(cx);
        cx.notify();
    }

    fn place(&self) -> Option<&Place> {
        self.places.get(&self.current?)
    }

    fn place_mut(&mut self) -> Option<&mut Place> {
        self.places.get_mut(&self.current?)
    }

    fn showing(&self, cx: &Context<Self>) -> bool {
        self.store.read(cx).dock == Some(PanelKind::Files)
    }

    fn tick(&mut self, list: bool, cx: &mut Context<Self>) {
        if !self.showing(cx) {
            return;
        }
        if list {
            self.relist(cx);
        }
        self.check_file(cx);
    }

    // -----------------------------------------------------------------------------------------
    // The tree.

    /// Lists the root and the open folders again, one job at a time.
    fn relist(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.current else { return };
        let Some(place) = self.places.get_mut(&id) else { return };
        if place.listing {
            return;
        }
        place.listing = true;
        let folders = tree::shown_folders(&place.expanded);
        self.list(id, folders, cx);
    }

    fn list(&mut self, id: u64, folders: Vec<PathBuf>, cx: &mut Context<Self>) {
        let Some(place) = self.places.get(&id) else { return };
        let root = place.root.clone();
        let job = cx.background_spawn(async move {
            folders
                .into_iter()
                .map(|rel| {
                    let listing = tree::list(&root, &rel);
                    (rel, listing)
                })
                .collect::<Vec<_>>()
        });
        cx.spawn(async move |this, cx| {
            let listed = job.await;
            let _ = this.update(cx, |this, cx| {
                let Some(place) = this.places.get_mut(&id) else { return };
                place.listing = false;
                let mut changed = false;
                for (rel, listing) in listed {
                    match listing {
                        Ok(listing) => {
                            if rel.as_os_str().is_empty() && place.error.take().is_some() {
                                changed = true;
                            }
                            if place.listings.get(&rel) != Some(&listing) {
                                place.listings.insert(rel, listing);
                                changed = true;
                            }
                        }
                        Err(e) => {
                            if rel.as_os_str().is_empty() {
                                place.error = Some(e.to_string());
                                changed = true;
                            } else {
                                // A folder that went away closes.
                                changed |= place.listings.remove(&rel).is_some() | place.expanded.remove(&rel);
                            }
                        }
                    }
                }
                if changed {
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn click_row(&mut self, rel: PathBuf, dir: bool, cx: &mut Context<Self>) {
        self.menu = None;
        if !dir {
            self.open_file(rel, None, None, false, cx);
            return;
        }
        let Some(id) = self.current else { return };
        let Some(place) = self.places.get_mut(&id) else { return };
        if !place.expanded.remove(&rel) {
            place.expanded.insert(rel.clone());
            self.list(id, vec![rel], cx);
        }
        cx.notify();
    }

    fn collapse_all(&mut self, cx: &mut Context<Self>) {
        if let Some(place) = self.place_mut() {
            place.expanded.clear();
        }
        cx.notify();
    }

    // -----------------------------------------------------------------------------------------
    // The viewer.

    /// Opens a file of the workspace (relative path) in the viewer, at a 0-based line.
    fn open_file(
        &mut self,
        rel: PathBuf,
        line: Option<usize>,
        lit: Option<RegexMatcher>,
        from_search: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.current else { return };
        let Some(place) = self.places.get_mut(&id) else { return };
        let path = place.root.join(&rel);
        place.last = Some(rel.clone());
        let opened = tree::reveal(&mut place.expanded, &rel);
        place.viewer = Some(Viewer {
            path: path.clone(),
            rel,
            content: None,
            spans: None,
            language: None,
            stamp: None,
            selected: line,
            scroll_to: Some(line.unwrap_or(0)),
            lit,
            checking: true,
            from_search,
        });
        if !opened.is_empty() {
            self.list(id, opened, cx);
        }
        self.read_file(id, path, true, cx);
        cx.notify();
    }

    /// Reads the open file off the main thread. The first read shows plain text at once and the
    /// colours after; a re-read answers only once coloured, so colours never flash.
    fn read_file(&mut self, id: u64, path: PathBuf, first: bool, cx: &mut Context<Self>) {
        let read_path = path.clone();
        let read = cx.background_spawn(async move { disk::read(&read_path) });
        cx.spawn(async move |this, cx| {
            let (content, stamp) = read.await;
            let lines = match &content {
                Content::Text(text) => Some(text.lines.clone()),
                _ => None,
            };
            let language =
                lines.as_ref().and_then(|l| highlight::language(&path, l.first().map_or("", String::as_str)));
            let apply = |this: &mut Self,
                         content: Option<Content>,
                         spans: Option<Arc<Vec<Vec<highlight::Span>>>>,
                         cx: &mut Context<Self>| {
                let Some(viewer) = this.places.get_mut(&id).and_then(|p| p.viewer.as_mut()) else { return };
                if viewer.path != path {
                    return;
                }
                if let Some(content) = content {
                    // A line past the end (the file shrank) selects nothing.
                    if let Content::Text(text) = &content
                        && viewer.selected.is_some_and(|l| l >= text.lines.len())
                    {
                        viewer.selected = None;
                    }
                    viewer.content = Some(content);
                    viewer.stamp = stamp;
                    viewer.language = language;
                }
                viewer.spans = spans;
                viewer.checking = false;
                if let (Some(line), Some(Content::Text(_))) = (viewer.scroll_to.take(), &viewer.content) {
                    this.code_scroll.scroll_to_item(line.saturating_sub(3), ScrollStrategy::Top);
                }
                cx.notify();
            };
            let Some(lines) = lines else {
                let _ = this.update(cx, |this, cx| apply(this, Some(content), None, cx));
                return;
            };
            let mut content = Some(content);
            if first {
                let _ = this.update(cx, |this, cx| apply(this, content.take(), None, cx));
            }
            let spans = cx.background_spawn({
                let path = path.clone();
                async move { highlight::highlight(&path, &lines).map(Arc::new) }
            });
            let spans = spans.await;
            let _ = this.update(cx, |this, cx| apply(this, content, spans, cx));
        })
        .detach();
    }

    /// Reads the open file again when its size or modification time changed.
    fn check_file(&mut self, cx: &mut Context<Self>) {
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
            let stamp = stamp.await;
            let _ = this.update(cx, |this, cx| {
                if stamp == known {
                    if let Some(viewer) = this.places.get_mut(&id).and_then(|p| p.viewer.as_mut()) {
                        viewer.checking = false;
                    }
                } else {
                    this.read_file(id, path, false, cx);
                }
            });
        })
        .detach();
    }

    fn close_file(&mut self, cx: &mut Context<Self>) {
        self.menu = None;
        let show_hidden = self.show_hidden;
        let Some(place) = self.place_mut() else { return };
        place.viewer = None;
        let last = place.last.clone();
        // Back on the tree, the file's row shows.
        let rows = tree::rows(&place.listings, &place.expanded, show_hidden);
        if let Some(ix) = last.and_then(|last| rows.iter().position(|r| r.path == last)) {
            self.tree_scroll.scroll_to_item(ix, ScrollStrategy::Center);
        }
        cx.notify();
    }

    fn select_line(&mut self, line: usize, cx: &mut Context<Self>) {
        if let Some(viewer) = self.place_mut().and_then(|p| p.viewer.as_mut()) {
            viewer.selected = if viewer.selected == Some(line) { None } else { Some(line) };
        }
        cx.notify();
    }

    // -----------------------------------------------------------------------------------------
    // The search.

    fn query_changed(&mut self, text: String, cx: &mut Context<Self>) {
        if let Some(place) = self.place_mut() {
            place.query = text;
            place.viewer = None;
        }
        self.search.enter = false;
        self.run_search(cx);
    }

    fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        if self.search.mode != mode {
            self.search.mode = mode;
            self.search.results = Results::None;
            self.search.rows.clear();
            self.run_search(cx);
        }
    }

    fn run_search(&mut self, cx: &mut Context<Self>) {
        if let Some(running) = self.search.running.take() {
            running.store(true, Ordering::Relaxed);
        }
        self.search.generation += 1;
        let query = self.place().map(|p| p.query.trim().to_string()).unwrap_or_default();
        let Some(root) = self.place().map(|p| p.root.clone()) else { return };
        if query.is_empty() {
            self.search.results = Results::None;
            self.search.rows.clear();
            self.search.answered.clear();
            cx.notify();
            return;
        }
        let generation = self.search.generation;
        let cancel = Arc::new(AtomicBool::new(false));
        self.search.running = Some(cancel.clone());
        let index = self
            .search
            .index
            .as_ref()
            .filter(|(r, _, at)| *r == root && at.elapsed() < INDEX_FRESH)
            .map(|(_, paths, _)| paths.clone());
        let mode = self.search.mode;
        let job = cx.background_spawn(async move {
            let (paths, fresh) = match index {
                Some(paths) => (paths, false),
                None => (Arc::new(search::index(&root, &cancel)), true),
            };
            let results = match mode {
                Mode::Names => Results::Names(search::names(&paths, &query)),
                Mode::Text => Results::Text(search::text(&root, &paths, &query, &cancel)),
            };
            (root, paths, fresh, query, results)
        });
        cx.spawn(async move |this, cx| {
            let (root, paths, fresh, query, results) = job.await;
            let _ = this.update(cx, |this, cx| {
                if fresh && this.place().is_some_and(|p| p.root == root) {
                    this.search.index = Some((root, paths, Instant::now()));
                }
                if this.search.generation != generation {
                    return;
                }
                this.search.running = None;
                this.search.answered = query;
                this.search.rows = match &results {
                    Results::Text(text) => text_rows(text),
                    _ => Vec::new(),
                };
                this.search.results = results;
                this.results_scroll.scroll_to_item(0, ScrollStrategy::Top);
                if std::mem::take(&mut this.search.enter) {
                    this.open_result(0, cx);
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// Enter in the field: opens the first result, once the answer to what was typed is there.
    fn enter(&mut self, cx: &mut Context<Self>) {
        if self.search.running.is_some() {
            self.search.enter = true;
        } else {
            self.open_result(0, cx);
        }
    }

    /// Opens the result of a row of the results list.
    fn open_result(&mut self, ix: usize, cx: &mut Context<Self>) {
        match &self.search.results {
            Results::None => {}
            Results::Names(hits) => {
                if let Some(hit) = hits.get(ix) {
                    let rel = PathBuf::from(&hit.path);
                    self.open_file(rel, None, None, true, cx);
                }
            }
            Results::Text(text) => {
                let target = match self.search.rows.get(ix) {
                    Some(TextRow::File(f)) => {
                        text.files.get(*f).map(|f| (f.path.clone(), f.lines.first().map(|l| l.line)))
                    }
                    Some(TextRow::Line(f, l)) => {
                        text.files.get(*f).and_then(|f| f.lines.get(*l).map(|l| (f.path.clone(), Some(l.line))))
                    }
                    None => None,
                };
                if let Some((path, line)) = target {
                    let lit = search::matcher(&self.search.answered);
                    let line = line.map(|l| (l as usize).saturating_sub(1));
                    self.open_file(PathBuf::from(path), line, lit, true, cx);
                }
            }
        }
    }

    // -----------------------------------------------------------------------------------------
    // Opening elsewhere and copying.

    fn show_menu(&mut self, at: Point<Pixels>, rel: PathBuf, dir: bool, line: Option<usize>, cx: &mut Context<Self>) {
        let Some(place) = self.place() else { return };
        self.menu = Some(Menu { at, path: place.root.join(&rel), rel, dir, line });
        cx.notify();
    }

    fn open_with(
        &mut self,
        opener: Opener,
        launch: Launch,
        path: PathBuf,
        line: Option<usize>,
        dir: bool,
        cx: &mut Context<Self>,
    ) {
        self.menu = None;
        cx.notify();
        let job = cx.background_spawn(async move {
            match open::argv(opener, &launch, &path, line.map(|l| l + 1), dir) {
                Some(argv) => open::run(&argv),
                None => Err("not a file it opens".to_string()),
            }
        });
        let store = self.store.clone();
        cx.spawn(async move |_, cx| {
            if let Err(e) = job.await {
                let _ = store.update(cx, |s, cx| {
                    s.toast(format!("could not open in {}: {e}", opener.label()), ToastKind::Error, cx)
                });
            }
        })
        .detach();
    }

    fn copy(&mut self, text: String, what: &str, cx: &mut Context<Self>) {
        self.menu = None;
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.store.update(cx, |s, cx| s.toast(format!("Copied {what}"), ToastKind::Info, cx));
        cx.notify();
    }

    // -----------------------------------------------------------------------------------------
    // Drawing.

    fn render_bar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let focused = self.query.read(cx).focus_handle(cx).is_focused(window);
        let mode = self.search.mode;
        let searching = !self.query.read(cx).text().trim().is_empty();
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(6.))
            .px(px(10.))
            .py(px(8.))
            .border_b_1()
            .border_color(colours::border())
            .child(div().flex_1().min_w_0().child(ui::input_box(self.query.clone(), focused)))
            .child(ui::segments(vec![
                ui::segment("files-mode-names", "Name", mode == Mode::Names)
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.set_mode(Mode::Names, cx)))
                    .into_any_element(),
                ui::segment("files-mode-text", "Text", mode == Mode::Text)
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.set_mode(Mode::Text, cx)))
                    .into_any_element(),
            ]))
            .when(!searching, |d| {
                d.child(ui::segments(vec![
                    ui::segment("files-hidden", "Hidden", self.show_hidden)
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.show_hidden = !this.show_hidden;
                            cx.notify();
                        }))
                        .into_any_element(),
                ]))
            })
    }

    fn render_tree(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(place) = self.place() else { return ui::note("No workspace shown").into_any_element() };
        if let Some(error) = &place.error {
            return ui::note(format!("Cannot read {}: {error}", place.root.display())).into_any_element();
        }
        let rows = tree::rows(&place.listings, &place.expanded, self.show_hidden);
        let loading = !place.listings.contains_key(Path::new(""));
        let truncated = place.listings.values().any(|l| l.truncated);
        let any_open = !place.expanded.is_empty();
        let root_label = home_relative(&place.root);
        self.rows = rows;
        let count = self.rows.len();
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.))
                    .px(px(12.))
                    .pt(px(6.))
                    .pb(px(4.))
                    .child(ui::dim(root_label).min_w_0().overflow_hidden().whitespace_nowrap().text_ellipsis())
                    .child(div().flex().flex_none().gap(px(4.)).items_center().when(any_open, |d| {
                        d.child(
                            ui::icon_button("files-collapse", "⊟")
                                .text_size(px(12.))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.collapse_all(cx))),
                        )
                    })),
            )
            .when(loading, |d| d.child(ui::note("Reading…")))
            .when(!loading && count == 0, |d| d.child(ui::note("Empty folder")))
            .when(truncated, |d| {
                d.child(
                    ui::dim(format!(
                        "Some folders hold more than {} entries; the rest are not listed",
                        tree::MAX_ENTRIES
                    ))
                    .px(px(12.)),
                )
            })
            .child(
                div().flex_1().min_h_0().px(px(6.)).pb(px(6.)).child(
                    uniform_list(
                        "files-tree",
                        count,
                        cx.processor(|this, range: Range<usize>, _window, cx| this.tree_items(range, cx)),
                    )
                    .size_full()
                    .track_scroll(self.tree_scroll.clone()),
                ),
            )
            .into_any_element()
    }

    fn tree_items(&mut self, range: Range<usize>, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let last = self.place().and_then(|p| p.last.clone());
        let menu_rel = self.menu.as_ref().map(|m| m.rel.clone());
        range
            .filter_map(|ix| self.rows.get(ix).cloned().map(|row| (ix, row)))
            .map(|(ix, row)| {
                let selected = last.as_ref() == Some(&row.path) || menu_rel.as_ref() == Some(&row.path);
                let (rel, dir) = (row.path.clone(), row.dir);
                let menu_path = row.path.clone();
                let chevron = if !row.dir {
                    ""
                } else if row.open {
                    "▾"
                } else {
                    "▸"
                };
                div()
                    .id(("files-row", ix))
                    .flex()
                    .items_center()
                    .h(px(24.))
                    .pl(px(6. + row.depth as f32 * 14.))
                    .pr(px(8.))
                    .rounded(px(4.))
                    .cursor_pointer()
                    .when(selected, |d| d.bg(colours::selected()))
                    .hover(|s| s.bg(colours::hover()))
                    .child(div().w(px(14.)).flex_none().text_size(px(12.)).text_color(colours::muted()).child(chevron))
                    .child(file_icon(&row.name, row.dir, row.open))
                    .child(
                        div()
                            .ml(px(6.))
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_color(if row.name.starts_with('.') { colours::muted() } else { colours::text() })
                            .child(row.name.clone()),
                    )
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.click_row(rel.clone(), dir, cx)))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                            this.show_menu(e.position, menu_path.clone(), dir, None, cx);
                        }),
                    )
                    .into_any_element()
            })
            .collect()
    }

    fn render_results(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let running = self.search.running.is_some();
        let (count, summary) = match &self.search.results {
            Results::None => (0, String::new()),
            Results::Names(hits) => (
                hits.len(),
                match hits.len() {
                    0 => "No file names match".to_string(),
                    n if n >= search::MAX_NAMES => format!("The best {n} files"),
                    1 => "1 file".to_string(),
                    n => format!("{n} files"),
                },
            ),
            Results::Text(text) => {
                let matches = text.count();
                let files = text.files.len();
                (
                    self.search.rows.len(),
                    if matches == 0 {
                        "No matches".to_string()
                    } else if text.capped {
                        format!("The first {matches} matches, in {files} files")
                    } else {
                        format!(
                            "{matches} {} in {files} {}",
                            plural(matches, "match", "matches"),
                            plural(files, "file", "files")
                        )
                    },
                )
            }
        };
        let summary = if running { "Searching…".to_string() } else { summary };
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(div().flex_none().px(px(12.)).pt(px(6.)).pb(px(4.)).child(ui::dim(summary)))
            .child(
                div().flex_1().min_h_0().px(px(6.)).pb(px(6.)).child(
                    uniform_list(
                        "files-results",
                        count,
                        cx.processor(|this, range: Range<usize>, _window, cx| this.result_items(range, cx)),
                    )
                    .size_full()
                    .track_scroll(self.results_scroll.clone()),
                ),
            )
            .into_any_element()
    }

    fn result_items(&mut self, range: Range<usize>, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let lit = HighlightStyle {
            color: Some(colours::accent().into()),
            font_weight: Some(FontWeight::BOLD),
            ..Default::default()
        };
        let mut items = Vec::new();
        for ix in range {
            let open = cx.listener(move |this, _: &ClickEvent, _, cx| this.open_result(ix, cx));
            let item = match &self.search.results {
                Results::None => continue,
                Results::Names(hits) => {
                    let Some(hit) = hits.get(ix) else { continue };
                    let split = hit.path.rfind('/').map_or(0, |i| i + 1);
                    let (folder, name) = hit.path.split_at(split);
                    let name_lit: Vec<_> =
                        clip(&hit.lit, split..hit.path.len()).into_iter().map(|r| (r, lit)).collect();
                    let folder_lit: Vec<_> = clip(&hit.lit, 0..split).into_iter().map(|r| (r, lit)).collect();
                    let rel = PathBuf::from(&hit.path);
                    result_row(("files-name", ix))
                        .child(file_icon(name, false, false))
                        .child(
                            div()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(StyledText::new(name.to_string()).with_highlights(name_lit)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_size(px(11.))
                                .text_color(colours::faint())
                                .child(
                                    StyledText::new(folder.trim_end_matches('/').to_string())
                                        .with_highlights(folder_lit),
                                ),
                        )
                        .on_click(open)
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                                this.show_menu(e.position, rel.clone(), false, None, cx)
                            }),
                        )
                        .into_any_element()
                }
                Results::Text(text) => match self.search.rows.get(ix) {
                    Some(TextRow::File(f)) => {
                        let Some(file) = text.files.get(*f) else { continue };
                        let name = file.path.rsplit('/').next().unwrap_or(&file.path).to_string();
                        let rel = PathBuf::from(&file.path);
                        result_row(("files-hit-file", ix))
                            .mt(px(if ix == 0 { 0. } else { 6. }))
                            .child(file_icon(&name, false, false))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(file.path.clone()),
                            )
                            .child(ui::dim(file.lines.len().to_string()).flex_none())
                            .on_click(open)
                            .on_mouse_down(
                                MouseButton::Right,
                                cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                                    this.show_menu(e.position, rel.clone(), false, None, cx)
                                }),
                            )
                            .into_any_element()
                    }
                    Some(TextRow::Line(f, l)) => {
                        let Some(hit) = text.files.get(*f).and_then(|f| f.lines.get(*l)) else { continue };
                        let lit_bg = HighlightStyle {
                            background_color: Some(with_alpha(hsla(ANSI[3]), 0.28)),
                            color: Some(colours::text().into()),
                            ..Default::default()
                        };
                        let spans: Vec<_> = hit.lit.iter().map(|r| (r.clone(), lit_bg)).collect();
                        result_row(("files-hit-line", ix))
                            .pl(px(28.))
                            .font_family(FONT_FAMILY)
                            .text_size(px(CODE_SIZE))
                            .child(
                                div().w(px(38.)).flex_none().text_color(colours::faint()).child(hit.line.to_string()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .text_color(colours::muted())
                                    .child(StyledText::new(hit.text.clone()).with_highlights(spans)),
                            )
                            .on_click(open)
                            .into_any_element()
                    }
                    None => continue,
                },
            };
            items.push(item);
        }
        items
    }

    fn render_viewer(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(viewer) = self.place().and_then(|p| p.viewer.as_ref()) else { return div().into_any_element() };
        let name = viewer
            .rel
            .file_name()
            .map_or_else(|| viewer.rel.display().to_string(), |n| n.to_string_lossy().into_owned());
        let folder = viewer.rel.parent().map(|p| p.display().to_string()).unwrap_or_default();
        let back = if viewer.from_search { "‹ Results" } else { "‹ Files" };
        let size = viewer.stamp.map(|s| disk::human_size(s.len));
        let mut info: Vec<String> = Vec::new();
        match &viewer.content {
            None => info.push("Reading…".into()),
            Some(Content::Text(text)) => {
                let n = text.lines.len();
                info.push(format!("{n} {}", plural(n, "line", "lines")));
                info.extend(size);
                info.extend(viewer.language.map(str::to_string));
                if let Some(line) = viewer.selected {
                    info.push(format!("line {}", line + 1));
                }
            }
            Some(Content::Image(image)) => {
                info.push(format!("{:?} image", image.format).to_uppercase().replace(" IMAGE", " image"));
                info.extend(size);
            }
            Some(_) => info.extend(size),
        }
        let (path, rel, line) = (viewer.path.clone(), viewer.rel.clone(), viewer.selected);
        let header = div()
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(6.))
            .px(px(10.))
            .py(px(8.))
            .border_b_1()
            .border_color(colours::border())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        ui::button("files-back", back)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close_file(cx))),
                    )
                    .child(file_icon(&name, false, false))
                    .child(div().flex_none().font_weight(FontWeight::SEMIBOLD).child(name.clone()))
                    .child(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(px(11.))
                            .text_color(colours::faint())
                            .child(folder),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .child(ui::dim(info.join("  ·  "))),
                    )
                    .child(ui::button("files-open-in", "Open in…").on_click(cx.listener({
                        let rel = rel.clone();
                        move |this, e: &ClickEvent, _, cx| this.show_menu(e.position(), rel.clone(), false, line, cx)
                    })))
                    .child(ui::button("files-copy-path", "Copy path").on_click(cx.listener({
                        let path = path.clone();
                        move |this, _: &ClickEvent, _, cx| this.copy(path.display().to_string(), "path", cx)
                    }))),
            );
        let body = match &viewer.content {
            None => ui::note("Reading…").into_any_element(),
            Some(Content::Text(text)) => {
                let count = text.lines.len();
                uniform_list(
                    "files-code",
                    count,
                    cx.processor(|this, range: Range<usize>, _window, cx| this.code_items(range, cx)),
                )
                .size_full()
                .py(px(4.))
                .track_scroll(self.code_scroll.clone())
                .into_any_element()
            }
            Some(Content::Image(image)) => div()
                .id("files-image")
                .size_full()
                .p(px(16.))
                .flex()
                .items_center()
                .justify_center()
                .child(img(image.clone()).max_w_full().max_h_full().object_fit(ObjectFit::ScaleDown))
                .into_any_element(),
            Some(Content::Binary) => {
                centered_note("Binary file", "Open it in another app to see it.").into_any_element()
            }
            Some(Content::TooLarge) => centered_note(
                "Too large to show",
                format!("Files over {} are not read here.", disk::human_size(disk::MAX_BYTES)),
            )
            .into_any_element(),
            Some(Content::NotRegular) => {
                centered_note("Not a regular file", "Devices, sockets and pipes are not read.").into_any_element()
            }
            Some(Content::Gone(e)) => centered_note("Cannot read this file", e.clone()).into_any_element(),
        };
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(header)
            .child(div().flex_1().min_h_0().bg(colours::terminal()).child(body))
            .into_any_element()
    }

    fn code_items(&mut self, range: Range<usize>, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(viewer) = self.place().and_then(|p| p.viewer.as_ref()) else { return Vec::new() };
        let Some(Content::Text(text)) = &viewer.content else { return Vec::new() };
        let digits = text.lines.len().to_string().len().max(3);
        let number_width = px(digits as f32 * 7.3 + 16.);
        let lit_bg = with_alpha(hsla(ANSI[3]), 0.3);
        range
            .filter_map(|ix| text.lines.get(ix).map(|line| (ix, line)))
            .map(|(ix, line)| {
                let selected = viewer.selected == Some(ix);
                let mut styles: Vec<(Range<usize>, HighlightStyle)> = viewer
                    .spans
                    .as_ref()
                    .and_then(|s| s.get(ix))
                    .map(|spans| spans.iter().map(|(r, k)| (r.clone(), kind_style(*k))).collect())
                    .unwrap_or_default();
                if let Some(matcher) = &viewer.lit {
                    styles = overlay(styles, matches(matcher, line), lit_bg);
                }
                let shown = if line.is_empty() { " ".to_string() } else { line.clone() };
                div()
                    .id(("files-line", ix))
                    .flex()
                    .h(px(18.))
                    .font_family(FONT_FAMILY)
                    .text_size(px(CODE_SIZE))
                    .items_center()
                    .whitespace_nowrap()
                    .when(selected, |d| d.bg(colours::selected()))
                    .hover(|s| s.bg(colours::panel()))
                    .child(
                        div()
                            .w(number_width)
                            .flex_none()
                            .pr(px(10.))
                            .text_right()
                            .text_color(if selected { colours::accent() } else { colours::faint() })
                            .child((ix + 1).to_string()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis()
                            .pr(px(8.))
                            .child(StyledText::new(shown).with_highlights(styles)),
                    )
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.select_line(ix, cx)))
                    .into_any_element()
            })
            .collect()
    }

    fn render_menu(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let menu = self.menu.as_ref()?;
        let mut entries: Vec<AnyElement> = Vec::new();
        for (i, (opener, launch)) in self.openers.iter().enumerate() {
            // The default app takes plain files only; checking costs one stat.
            if *opener == Opener::DefaultApp && open::argv(*opener, launch, &menu.path, None, menu.dir).is_none() {
                continue;
            }
            let label = match opener {
                Opener::Finder if !menu.dir => "Reveal in Finder".to_string(),
                Opener::DefaultApp => "Open with default app".to_string(),
                other => format!("Open in {}", other.label()),
            };
            let (opener, launch, path, line, dir) = (*opener, launch.clone(), menu.path.clone(), menu.line, menu.dir);
            entries.push(
                menu_entry(("files-menu-open", i), label)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.open_with(opener, launch.clone(), path.clone(), line, dir, cx);
                    }))
                    .into_any_element(),
            );
        }
        if self.openers.is_empty() {
            entries.push(div().px(px(10.)).py(px(4.)).child(ui::dim("No apps to open with")).into_any_element());
        }
        let abs = menu.path.display().to_string();
        let rel = menu.rel.display().to_string();
        entries.push(div().my(px(4.)).h(px(1.)).bg(colours::border()).into_any_element());
        entries.push(
            menu_entry("files-menu-copy", "Copy path")
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.copy(abs.clone(), "path", cx)))
                .into_any_element(),
        );
        entries.push(
            menu_entry("files-menu-copy-rel", "Copy relative path")
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.copy(rel.clone(), "relative path", cx)))
                .into_any_element(),
        );
        Some(
            deferred(
                anchored().position(menu.at).snap_to_window_with_margin(px(8.)).child(
                    div()
                        .id("files-menu")
                        .min_w(px(200.))
                        .p(px(4.))
                        .rounded(px(6.))
                        .bg(colours::panel())
                        .border_1()
                        .border_color(colours::border())
                        .shadow_lg()
                        .text_size(px(13.))
                        .children(entries)
                        .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, _, cx| {
                            this.menu = None;
                            cx.notify();
                        })),
                ),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }
}

impl Render for FilesPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(place) = self.place() else {
            return div().size_full().child(ui::note("No workspace shown")).into_any_element();
        };
        let viewing = place.viewer.is_some();
        let searching = !place.query.trim().is_empty();
        let body = if viewing {
            self.render_viewer(cx)
        } else if searching {
            self.render_results(cx)
        } else {
            self.render_tree(cx)
        };
        let menu = self.render_menu(cx);
        div()
            .size_full()
            .flex()
            .flex_col()
            .when(!viewing, |d| d.child(self.render_bar(window, cx)))
            .child(body)
            .children(menu)
            .into_any_element()
    }
}

pub(crate) fn build(window: &mut Window, cx: &mut gpui::App) -> AnyView {
    cx.new(|cx| FilesPanel::new(window, cx)).into()
}

// ---------------------------------------------------------------------------------------------
// Helpers.

/// The text results as rows: each file, then its lines.
fn text_rows(text: &search::TextResults) -> Vec<TextRow> {
    let mut rows = Vec::new();
    for (f, file) in text.files.iter().enumerate() {
        rows.push(TextRow::File(f));
        rows.extend((0..file.lines.len()).map(|l| TextRow::Line(f, l)));
    }
    rows
}

/// The parts of `ranges` inside `within`, relative to its start.
fn clip(ranges: &[Range<usize>], within: Range<usize>) -> Vec<Range<usize>> {
    ranges
        .iter()
        .filter_map(|r| {
            let (start, end) = (r.start.max(within.start), r.end.min(within.end));
            (start < end).then(|| start - within.start..end - within.start)
        })
        .collect()
}

/// Byte ranges of a matcher's occurrences in a line.
fn matches(matcher: &RegexMatcher, line: &str) -> Vec<Range<usize>> {
    let mut found = Vec::new();
    let _ = matcher.find_iter(line.as_bytes(), |m| {
        found.push(m.start()..m.end());
        true
    });
    found
}

/// Puts a background on `lit` over syntax styles, splitting them where they cross, so the runs
/// stay sorted and apart as `StyledText` wants them.
fn overlay(
    styles: Vec<(Range<usize>, HighlightStyle)>,
    lit: Vec<Range<usize>>,
    bg: Hsla,
) -> Vec<(Range<usize>, HighlightStyle)> {
    if lit.is_empty() {
        return styles;
    }
    let mut cuts: Vec<usize> =
        styles.iter().flat_map(|(r, _)| [r.start, r.end]).chain(lit.iter().flat_map(|r| [r.start, r.end])).collect();
    cuts.sort_unstable();
    cuts.dedup();
    let mut out: Vec<(Range<usize>, HighlightStyle)> = Vec::new();
    for pair in cuts.windows(2) {
        let piece = pair[0]..pair[1];
        let base = styles.iter().find(|(r, _)| r.start <= piece.start && piece.end <= r.end).map(|(_, s)| *s);
        let in_lit = lit.iter().any(|r| r.start <= piece.start && piece.end <= r.end);
        let style = match (base, in_lit) {
            (None, false) => continue,
            (Some(s), false) => s,
            (base, true) => HighlightStyle { background_color: Some(bg), ..base.unwrap_or_default() },
        };
        out.push((piece, style));
    }
    out
}

fn kind_style(kind: Kind) -> HighlightStyle {
    let colour = |i: usize| Some(hsla(ANSI[i]));
    match kind {
        Kind::Keyword => HighlightStyle { color: colour(5), ..Default::default() },
        Kind::Function => HighlightStyle { color: colour(4), ..Default::default() },
        Kind::Type => HighlightStyle { color: colour(6), ..Default::default() },
        Kind::String => HighlightStyle { color: colour(2), ..Default::default() },
        Kind::Number => HighlightStyle { color: colour(3), ..Default::default() },
        Kind::Tag => HighlightStyle { color: colour(1), ..Default::default() },
        Kind::Comment => HighlightStyle {
            color: Some(colours::faint().into()),
            font_style: Some(FontStyle::Italic),
            ..Default::default()
        },
    }
}

fn with_alpha(mut colour: Hsla, alpha: f32) -> Hsla {
    colour.a = alpha;
    colour
}

fn plural<'a>(n: usize, one: &'a str, many: &'a str) -> &'a str {
    if n == 1 { one } else { many }
}

/// `~/…` for a folder under the home folder.
fn home_relative(path: &Path) -> String {
    match std::env::var_os("HOME").map(PathBuf::from) {
        Some(home) if path.starts_with(&home) => {
            format!("~/{}", path.strip_prefix(&home).map(|p| p.display().to_string()).unwrap_or_default())
        }
        _ => path.display().to_string(),
    }
}

/// A small coloured glyph for a file's kind, in a fixed-width slot so names line up.
fn file_icon(name: &str, dir: bool, open: bool) -> impl IntoElement + use<> {
    let (glyph, colour) = if dir {
        (if open { "▫" } else { "▪" }, Hsla::from(colours::accent()))
    } else {
        let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
        let colour = match ext.as_str() {
            "rs" => hsla([222, 165, 132]),
            "ts" | "tsx" | "mts" | "cts" => hsla(ANSI[4]),
            "js" | "jsx" | "mjs" | "cjs" => hsla(ANSI[3]),
            "py" => hsla(ANSI[6]),
            "go" => hsla(ANSI[6]),
            "md" | "markdown" | "txt" => hsla(ANSI[7]),
            "json" | "toml" | "yaml" | "yml" | "lock" => hsla(ANSI[3]),
            "html" | "css" | "scss" | "svelte" | "vue" => hsla(ANSI[1]),
            "sh" | "zsh" | "bash" | "fish" => hsla(ANSI[2]),
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "ico" => hsla(ANSI[5]),
            _ => colours::faint().into(),
        };
        ("•", colour)
    };
    div().w(px(12.)).flex_none().flex().justify_center().text_size(px(12.)).text_color(colour).child(glyph)
}

fn result_row(id: impl Into<gpui::ElementId>) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(8.))
        .h(px(24.))
        .px(px(8.))
        .rounded(px(4.))
        .cursor_pointer()
        .hover(|s| s.bg(colours::hover()))
}

fn menu_entry(id: impl Into<gpui::ElementId>, label: impl Into<SharedString>) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px(px(10.))
        .py(px(4.))
        .rounded(px(4.))
        .cursor_pointer()
        .whitespace_nowrap()
        .hover(|s| s.bg(colours::hover()))
        .child(label.into())
}

fn centered_note(title: impl Into<SharedString>, detail: impl Into<SharedString>) -> gpui::Div {
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(4.))
        .child(div().text_color(colours::muted()).child(title.into()))
        .child(ui::dim(detail))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_splits_styles_under_lit_ranges() {
        let red = HighlightStyle { color: Some(hsla(ANSI[1])), ..Default::default() };
        let bg = hsla(ANSI[3]);
        let out = overlay(vec![(0..6, red)], vec![4..8], bg);
        let ranges: Vec<_> = out.iter().map(|(r, _)| r.clone()).collect();
        assert_eq!(ranges, vec![0..4, 4..6, 6..8]);
        assert_eq!(out[1].1.color, red.color);
        assert_eq!(out[1].1.background_color, Some(bg));
        assert_eq!(out[2].1.color, None);
        assert_eq!(overlay(vec![(0..2, red)], Vec::new(), bg).len(), 1);
    }

    #[test]
    fn clip_keeps_parts_inside() {
        assert_eq!(clip(&[0..3, 5..9], 4..8), vec![1..4]);
        assert!(clip(&[0..2], 2..4).is_empty());
    }

    #[test]
    fn text_rows_list_files_then_lines() {
        let hit = |line| search::LineHit { line, text: String::new(), lit: Vec::new() };
        let results = search::TextResults {
            files: vec![
                search::FileHits { path: "a".into(), lines: vec![hit(1), hit(2)] },
                search::FileHits { path: "b".into(), lines: vec![hit(3)] },
            ],
            capped: false,
        };
        let rows = text_rows(&results);
        assert_eq!(rows.len(), 5);
        assert!(matches!(rows[3], TextRow::File(1)));
        assert!(matches!(rows[4], TextRow::Line(1, 0)));
    }
}
