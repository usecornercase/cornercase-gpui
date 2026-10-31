mod view;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gpui::{
    AnyView, App, AppContext as _, Context, Entity, Focusable as _, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, Pixels, Render, ScrollStrategy, Size, Styled as _, Subscription, UniformListScrollHandle,
    WeakEntity, Window, px,
};

use crate::cli;
use crate::model::{Project, Workspace};
use crate::store::Store;
use crate::theme::radius;
use crate::ui::{TextField, TextFieldEvent, glass};

use super::querying::{Answer, Job};
use super::results_view::names_summary;
use super::search::{self, NameHit};
use super::source::Source;
use super::viewer::dock_files;
use super::workspace::shown_workspace;
use super::{FilesPanel, Mode, Results};

const WIDTH: f32 = 640.;
const ROW: f32 = 30.;
const CHROME: f32 = 112.;

struct Spot {
    id: u64,
    root: PathBuf,
    source: Source,
    label: String,
}

impl Spot {
    fn new(project: &Project, workspace: &Workspace) -> Self {
        let root = PathBuf::from(&workspace.path);
        let source = Source::new(workspace.id, root.clone(), cli::host().is_some());
        Self { id: workspace.id, root, source, label: place_label(project, workspace) }
    }
}

pub(super) fn place_label(project: &Project, workspace: &Workspace) -> String {
    format!("{} › {}", project.name, workspace.name)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Frame {
    pub(super) width: f32,
    pub(super) height: f32,
    pub(super) page: usize,
}

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "clamped to 1..=100 before the cast")]
fn whole_rows(space: f32) -> usize {
    (space / ROW).floor().clamp(1., 100.) as usize
}

pub(super) fn frame_for(viewport: Size<Pixels>) -> Frame {
    let width = WIDTH.min(f32::from(viewport.width) - 48.).max(320.);
    let height = (f32::from(viewport.height) - 96.).clamp(220., 440.);
    Frame { width, height, page: whole_rows(height - CHROME) }
}

pub(super) fn step_of(key: &str, page: usize) -> Option<isize> {
    let page = isize::try_from(page).unwrap_or(1);
    match key {
        "down" => Some(1),
        "up" => Some(-1),
        "pagedown" => Some(page),
        "pageup" => Some(-page),
        _ => None,
    }
}

#[derive(Default)]
pub(super) struct Pick {
    pub(super) hits: Vec<NameHit>,
    pub(super) failed: Option<String>,
    pub(super) selected: usize,
    pub(super) running: bool,
    generation: u64,
    enter: bool,
}

impl Pick {
    pub(super) fn ask(&mut self) -> u64 {
        self.generation += 1;
        self.running = true;
        self.generation
    }

    pub(super) fn clear(&mut self) {
        *self = Self { generation: self.generation + 1, ..Self::default() };
    }

    pub(super) fn retyped(&mut self) {
        self.enter = false;
    }

    pub(super) fn answer(&mut self, generation: u64, results: Results) -> Option<bool> {
        if generation != self.generation {
            return None;
        }
        self.running = false;
        self.selected = 0;
        (self.hits, self.failed) = match results {
            Results::Names(hits) => (hits, None),
            Results::Failed(e) => (Vec::new(), Some(e)),
            Results::None | Results::Text(_) => (Vec::new(), None),
        };
        Some(std::mem::take(&mut self.enter))
    }

    pub(super) fn submit(&mut self) -> Option<usize> {
        if self.running {
            self.enter = true;
            return None;
        }
        (self.selected < self.hits.len()).then_some(self.selected)
    }

    pub(super) fn step(&mut self, by: isize) -> bool {
        let Some(last) = self.hits.len().checked_sub(1) else { return false };
        let to = self.selected.saturating_add_signed(by).min(last);
        let moved = to != self.selected;
        self.selected = to;
        moved
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Status {
    Nowhere,
    Hint,
    Reading,
    Searching,
    Failed(String),
    Found(String),
}

pub(super) fn status(somewhere: bool, query: &str, warming: bool, pick: &Pick) -> Status {
    if !somewhere {
        return Status::Nowhere;
    }
    if query.trim().is_empty() {
        return Status::Hint;
    }
    if warming {
        return Status::Reading;
    }
    if pick.running {
        return Status::Searching;
    }
    match &pick.failed {
        Some(e) => Status::Failed(e.clone()),
        None => Status::Found(names_summary(&pick.hits)),
    }
}

pub(crate) struct Finder {
    panel: WeakEntity<FilesPanel>,
    field: Entity<TextField>,
    spot: Option<Spot>,
    index: Option<Arc<Vec<String>>>,
    warming: Option<Arc<AtomicBool>>,
    running: Option<Arc<AtomicBool>>,
    pick: Pick,
    page: usize,
    scroll: UniformListScrollHandle,
    _subscriptions: Vec<Subscription>,
}

pub(crate) fn find_file(files: AnyView, window: &mut Window, cx: &mut App) {
    let Ok(panel) = files.downcast::<FilesPanel>() else { return };
    let finder = cx.new(|cx| Finder::new(panel.downgrade(), cx));
    let focus = finder.read(cx).field.read(cx).focus_handle(cx);
    Store::get(cx).update(cx, |s, cx| s.show_dialog(finder.into(), cx));
    window.focus(&focus);
}

pub(crate) fn is_finder(view: &AnyView) -> bool {
    view.entity_type() == std::any::TypeId::of::<Finder>()
}

impl Finder {
    fn new(panel: WeakEntity<FilesPanel>, cx: &mut Context<Self>) -> Self {
        let field = cx.new(|cx| TextField::new("Go to a file…", cx).clears_on_escape());
        let store = Store::get(cx);
        let subscriptions = vec![
            cx.subscribe(&field, |this, _, event: &TextFieldEvent, cx| this.field_event(event, cx)),
            cx.observe(&store, |this, _, cx| this.follow(cx)),
        ];
        let mut finder = Self {
            panel,
            field,
            spot: None,
            index: None,
            warming: None,
            running: None,
            pick: Pick::default(),
            page: 1,
            scroll: UniformListScrollHandle::new(),
            _subscriptions: subscriptions,
        };
        finder.follow(cx);
        finder
    }

    fn field_event(&mut self, event: &TextFieldEvent, cx: &mut Context<Self>) {
        match event {
            TextFieldEvent::Changed => {
                self.pick.retyped();
                self.search(cx);
            }
            TextFieldEvent::Submit => self.submit(cx),
            TextFieldEvent::Cancel => {}
        }
    }

    fn query(&self, cx: &App) -> String {
        self.field.read(cx).text().trim().to_string()
    }

    fn follow(&mut self, cx: &mut Context<Self>) {
        let store = Store::get(cx);
        let shown = shown_workspace(store.read(cx).session.as_ref());
        let here = self.spot.as_ref().map(|s| (s.id, s.root.as_path()));
        if here == shown.map(|(_, ws)| (ws.id, Path::new(&ws.path))) {
            return;
        }
        let spot = shown.map(|(project, ws)| Spot::new(project, ws));
        self.stop();
        self.index = None;
        self.spot = spot;
        self.warm(cx);
        self.search(cx);
    }

    fn stop(&mut self) {
        for cancel in [self.warming.take(), self.running.take()].into_iter().flatten() {
            cancel.store(true, Ordering::Relaxed);
        }
    }

    fn warm(&mut self, cx: &mut Context<Self>) {
        let Some(Spot { root, source: Source::Disk(_), .. }) = &self.spot else { return };
        let root = root.clone();
        self.index = self.panel.read_with(cx, |panel, _| panel.fresh_index(&root)).ok().flatten();
        if self.index.is_some() {
            return;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.warming = Some(cancel.clone());
        let walking = (root.clone(), cancel.clone());
        let task = cx.background_spawn(async move { search::index(&walking.0, &walking.1) });
        cx.spawn(async move |this, cx| {
            let paths = task.await;
            let _ = this.update(cx, |this, cx| this.warmed(&cancel, root, paths, cx));
        })
        .detach();
    }

    fn warmed(&mut self, cancel: &Arc<AtomicBool>, root: PathBuf, paths: Option<Vec<String>>, cx: &mut Context<Self>) {
        if !self.warming.as_ref().is_some_and(|w| Arc::ptr_eq(w, cancel)) {
            return;
        }
        self.warming = None;
        if let Some(paths) = paths {
            self.keep(root, Arc::new(paths), cx);
        }
        self.search(cx);
    }

    fn keep(&mut self, root: PathBuf, paths: Arc<Vec<String>>, cx: &mut Context<Self>) {
        self.index = Some(paths.clone());
        let _ = self.panel.update(cx, |panel, _| panel.keep_index(root, paths));
    }

    fn search(&mut self, cx: &mut Context<Self>) {
        if let Some(cancel) = self.running.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        let query = self.query(cx);
        if query.is_empty() || self.spot.is_none() {
            self.pick.clear();
            cx.notify();
            return;
        }
        let generation = self.pick.ask();
        if self.warming.is_none() {
            self.run(query, generation, cx);
        }
        cx.notify();
    }

    fn run(&mut self, query: String, generation: u64, cx: &mut Context<Self>) {
        let Some(spot) = &self.spot else { return };
        let cancel = Arc::new(AtomicBool::new(false));
        self.running = Some(cancel.clone());
        let (root, source, index) = (spot.root.clone(), spot.source.clone(), self.index.clone());
        let job = Job { root, source, index, query, mode: Mode::Names, cancel };
        job.spawn(cx, move |this, answer, cx| this.answered(generation, answer, cx));
    }

    fn answered(&mut self, generation: u64, answer: Answer, cx: &mut Context<Self>) {
        if answer.fresh && self.spot.as_ref().is_some_and(|s| s.root == answer.root) {
            self.keep(answer.root, answer.paths, cx);
        }
        let Some(enter) = self.pick.answer(generation, answer.results) else { return };
        self.running = None;
        self.scroll.scroll_to_item(0, ScrollStrategy::Top);
        if enter {
            self.open(self.pick.selected, cx);
        }
        cx.notify();
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        if let Some(ix) = self.pick.submit() {
            self.open(ix, cx);
        }
    }

    fn open(&mut self, ix: usize, cx: &mut Context<Self>) {
        let (Some(spot), Some(hit)) = (&self.spot, self.pick.hits.get(ix)) else { return };
        let (id, root, rel) = (spot.id, spot.root.clone(), PathBuf::from(&hit.path));
        let _ = self.panel.update(cx, |panel, cx| panel.reveal(id, &root, rel, cx));
        dock_files(cx);
    }

    fn key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let m = event.keystroke.modifiers;
        if m.platform || m.control || m.alt {
            return;
        }
        let Some(by) = step_of(event.keystroke.key.as_str(), self.page) else { return };
        cx.stop_propagation();
        if self.pick.step(by) {
            let strategy = if by > 0 { ScrollStrategy::Bottom } else { ScrollStrategy::Top };
            self.scroll.scroll_to_item(self.pick.selected, strategy);
            cx.notify();
        }
    }

    fn status(&self, cx: &App) -> Status {
        status(self.spot.is_some(), &self.query(cx), self.warming.is_some(), &self.pick)
    }
}

impl Drop for Finder {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Render for Finder {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let frame = frame_for(window.viewport_size());
        self.page = frame.page;
        let status = self.status(cx);
        glass::card(radius::SHEET)
            .w(px(frame.width))
            .h(px(frame.height))
            .flex()
            .flex_col()
            .overflow_hidden()
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| this.key(e, cx)))
            .child(self.top(window, cx))
            .child(self.body(&status, cx))
            .child(self.footer(&status))
    }
}

#[cfg(test)]
mod tests;
