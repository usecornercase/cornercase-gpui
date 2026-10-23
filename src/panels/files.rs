mod disk;
mod highlight;
mod menu;
mod open;
mod querying;
mod results_view;
mod search;
mod style;
mod tree;
mod tree_view;
mod viewer;
mod viewer_view;
mod workspace;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use gpui::{
    AnyView, AppContext as _, ClickEvent, Context, Entity, Focusable as _, IntoElement, ParentElement as _, Pixels,
    Point, Render, StatefulInteractiveElement as _, Styled as _, Subscription, Task, UniformListScrollHandle, Window,
    div, prelude::FluentBuilder as _, px,
};
use grep_regex::RegexMatcher;

use crate::panels::PanelKind;
use crate::store::Store;
use crate::theme::colours;
use crate::ui::{self, TextField, TextFieldEvent};

use disk::Content;
use open::{Launch, Opener};

const CHECK_EVERY: Duration = Duration::from_secs(1);
const LIST_EVERY: u32 = 2;
const INDEX_FRESH: Duration = Duration::from_secs(10);
const CODE_SIZE: f32 = 12.;

type Spans = Arc<Vec<Vec<highlight::Span>>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Names,
    Text,
}

struct Place {
    root: PathBuf,
    listings: HashMap<PathBuf, tree::Listing>,
    error: Option<String>,
    expanded: HashSet<PathBuf>,
    viewer: Option<Viewer>,
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
    path: PathBuf,
    rel: PathBuf,
    content: Option<Content>,
    spans: Option<Spans>,
    language: Option<&'static str>,
    stamp: Option<disk::Stamp>,
    selected: Option<usize>,
    scroll_to: Option<usize>,
    lit: Option<RegexMatcher>,
    checking: bool,
    from_search: bool,
}

enum Results {
    None,
    Names(Vec<search::NameHit>),
    Text(search::TextResults),
}

#[derive(Clone, Copy)]
enum TextRow {
    File(usize),
    Line(usize, usize),
}

struct Search {
    mode: Mode,
    answered: String,
    results: Results,
    rows: Vec<TextRow>,
    running: Option<Arc<AtomicBool>>,
    generation: u64,
    enter: bool,
    index: Option<(PathBuf, Arc<Vec<String>>, Instant)>,
}

impl Search {
    fn new() -> Self {
        Self {
            mode: Mode::Names,
            answered: String::new(),
            results: Results::None,
            rows: Vec::new(),
            running: None,
            generation: 0,
            enter: false,
            index: None,
        }
    }

    fn clear(&mut self) {
        self.results = Results::None;
        self.rows.clear();
        self.answered.clear();
    }
}

#[derive(Clone)]
struct Target {
    rel: PathBuf,
    dir: bool,
    line: Option<usize>,
}

struct Menu {
    at: Point<Pixels>,
    path: PathBuf,
    target: Target,
}

pub(crate) struct FilesPanel {
    store: Entity<Store>,
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

fn ticker(cx: &mut Context<FilesPanel>) -> Task<()> {
    cx.spawn(async move |this, cx| {
        let mut n = 0u32;
        loop {
            cx.background_executor().timer(CHECK_EVERY).await;
            n = n.wrapping_add(1);
            let list = n.is_multiple_of(LIST_EVERY);
            if this.update(cx, |this, cx| this.tick(list, cx)).is_err() {
                break;
            }
        }
    })
}

fn detect_openers(cx: &mut Context<FilesPanel>) {
    let openers = cx.background_spawn(async { open::detect() });
    cx.spawn(async move |this, cx| {
        let openers = openers.await;
        let _ = this.update(cx, |this, _| this.openers = Arc::new(openers));
    })
    .detach();
}

fn query_event(this: &mut FilesPanel, field: &Entity<TextField>, event: &TextFieldEvent, cx: &mut Context<FilesPanel>) {
    match event {
        TextFieldEvent::Changed => {
            let text = field.read(cx).text();
            this.query_changed(text, cx);
        }
        TextFieldEvent::Submit => this.enter(cx),
        TextFieldEvent::Cancel => field.update(cx, TextField::clear),
    }
}

impl FilesPanel {
    pub(crate) fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = Store::get(cx);
        let query = cx.new(|cx| TextField::new("Search files…", cx));
        let subscriptions = vec![
            cx.observe(&store, |this, _, cx| this.follow_workspace(cx)),
            cx.subscribe(&query, |this, field, event: &TextFieldEvent, cx| query_event(this, &field, event, cx)),
        ];
        let tick = ticker(cx);
        detect_openers(cx);
        let mut panel = Self {
            store,
            current: None,
            places: HashMap::new(),
            query,
            search: Search::new(),
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

    fn place(&self) -> Option<&Place> {
        self.places.get(&self.current?)
    }

    fn place_mut(&mut self) -> Option<&mut Place> {
        self.places.get_mut(&self.current?)
    }

    fn showing(&self, cx: &Context<Self>) -> bool {
        self.store.read(cx).dock == Some(PanelKind::Files)
    }

    fn mode_segment(
        &self,
        id: &'static str,
        label: &'static str,
        mode: Mode,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        ui::segment(id, label, self.search.mode == mode)
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_mode(mode, cx)))
            .into_any_element()
    }

    fn hidden_toggle(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        ui::segments(vec![
            ui::segment("files-hidden", "Hidden", self.show_hidden)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.show_hidden = !this.show_hidden;
                    cx.notify();
                }))
                .into_any_element(),
        ])
    }

    fn render_bar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let focused = self.query.read(cx).focus_handle(cx).is_focused(window);
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
                self.mode_segment("files-mode-names", "Name", Mode::Names, cx),
                self.mode_segment("files-mode-text", "Text", Mode::Text, cx),
            ]))
            .when(!searching, |d| d.child(self.hidden_toggle(cx)))
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
