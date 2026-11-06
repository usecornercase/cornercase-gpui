pub(crate) mod disk;
mod find;
mod finder;
mod gutter;
mod gutter_view;
mod highlight;
mod lines;
mod menu;
pub(crate) mod open;
mod peek;
mod querying;
mod results_view;
mod search;
mod served;
mod source;
mod style;
mod there;
mod tree;
mod tree_view;
mod viewer;
mod viewer_view;
mod workspace;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use gpui::{
    AnyView, AppContext as _, ClickEvent, Context, Entity, FocusHandle, Focusable as _, InteractiveElement as _,
    IntoElement, ListState, ParentElement as _, Pixels, Point, Render, StatefulInteractiveElement as _, Styled as _,
    Subscription, Task, UniformListScrollHandle, Window, div, prelude::FluentBuilder as _, px,
};

use crate::cli;
use crate::panels::PanelKind;
use crate::panels::on_machine::Retry;
use crate::platform::Os;
use crate::platform::paths::Style;
use crate::prefs::LongLines;
use crate::store::{MachineRead, Store};
use crate::theme::colours;
use crate::ui::selection::Selecting;
use crate::ui::{self, TextField, TextFieldEvent};

use disk::Content;
pub(crate) use finder::{find_file, is_finder};
pub(crate) use peek::{Anchor, Peek, WebPage};
use source::Source;
pub(crate) use viewer::open_link;

const CHECK_EVERY: Duration = Duration::from_secs(1);
const LIST_EVERY: u32 = 2;
const CHANGES_EVERY: u32 = 10;
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
    style: Style,
    source: Source,
    listings: HashMap<PathBuf, tree::Listing>,
    marks: tree::Marks,
    error: Option<String>,
    expanded: HashSet<PathBuf>,
    viewer: Option<Viewer>,
    last: Option<PathBuf>,
    query: String,
    listing: bool,
    marking: bool,
}

impl Place {
    fn new(root: PathBuf, source: Source) -> Self {
        Self {
            style: Style::of_path(&root.to_string_lossy()),
            root,
            source,
            listings: HashMap::new(),
            marks: HashMap::new(),
            error: None,
            expanded: HashSet::new(),
            viewer: None,
            last: None,
            query: String::new(),
            listing: false,
            marking: false,
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
    selecting: Selecting<usize>,
    list: ListState,
    long_lines: LongLines,
    margin: gutter::Margin,
    reveal: Option<viewer::Reveal>,
    scroll_to: Option<usize>,
    find: Option<find::Find>,
    checking: bool,
    from_search: bool,
    from_flow: bool,
}

enum Results {
    None,
    Names(Vec<search::NameHit>),
    Text(search::TextResults),
    Failed(String),
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

    fn forget(&mut self) {
        self.clear();
        self.generation += 1;
        self.enter = false;
        if let Some(running) = self.running.take() {
            running.store(true, Ordering::Relaxed);
        }
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
    default_app: bool,
}

pub(crate) struct FilesPanel {
    store: Entity<Store>,
    remote: bool,
    machine: Option<MachineRead>,
    retry: Retry,
    current: Option<u64>,
    places: HashMap<u64, Place>,
    query: Entity<TextField>,
    focus: FocusHandle,
    left_place: bool,
    search: Search,
    show_hidden: bool,
    rows: Vec<tree::Row>,
    menu: Option<Menu>,
    tree_scroll: UniformListScrollHandle,
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
            let (list, again) = (n.is_multiple_of(LIST_EVERY), n.is_multiple_of(CHANGES_EVERY));
            if this.update(cx, |this, cx| this.tick(list, again, cx)).is_err() {
                break;
            }
        }
    })
}

fn query_event(this: &mut FilesPanel, field: &Entity<TextField>, event: &TextFieldEvent, cx: &mut Context<FilesPanel>) {
    match event {
        TextFieldEvent::Changed => {
            let text = field.read(cx).text();
            this.query_changed(text, cx);
        }
        TextFieldEvent::Submit => this.enter(cx),
        TextFieldEvent::Cancel => {}
    }
}

impl FilesPanel {
    pub(crate) fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = Store::get(cx);
        let query = cx.new(|cx| TextField::new("Search files…", cx).clears_on_escape());
        let subscriptions = vec![
            cx.observe(&store, |this, _, cx| this.store_changed(cx)),
            cx.subscribe(&query, |this, field, event: &TextFieldEvent, cx| query_event(this, &field, event, cx)),
        ];
        let tick = ticker(cx);
        let remote = cli::host().is_some();
        let mut panel = Self {
            store,
            remote,
            machine: None,
            retry: Retry::default(),
            current: None,
            places: HashMap::new(),
            query,
            focus: cx.focus_handle(),
            left_place: false,
            search: Search::new(),
            show_hidden: true,
            rows: Vec::new(),
            menu: None,
            tree_scroll: UniformListScrollHandle::new(),
            results_scroll: UniformListScrollHandle::new(),
            _subscriptions: subscriptions,
            _tick: tick,
        };
        panel.store_changed(cx);
        panel
    }

    fn place(&self) -> Option<&Place> {
        self.places.get(&self.current?)
    }

    fn style(&self) -> Style {
        self.place().map_or(Style::of(Os::CURRENT), |place| place.style)
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

    fn focus_lost_with_a_field(&mut self, window: &Window, cx: &mut Context<Self>) -> bool {
        let viewer = self.place().and_then(|p| p.viewer.as_ref());
        let query_hidden = viewer.is_some() && self.query.read(cx).focus_handle(cx).is_focused(window);
        let find_hidden = viewer
            .and_then(|v| v.find.as_ref())
            .filter(|_| !self.can_find())
            .is_some_and(|find| find.field.read(cx).focus_handle(cx).is_focused(window));
        let left_place = std::mem::take(&mut self.left_place) && self.focus.contains_focused(window, cx);
        query_hidden || find_hidden || left_place
    }

    fn settle_focus(&mut self, window: &Window, cx: &mut Context<Self>) {
        if !self.focus_lost_with_a_field(window, cx) {
            return;
        }
        cx.defer_in(window, |this, window, cx| {
            let viewer = this.place().and_then(|p| p.viewer.as_ref());
            let field = viewer.and_then(|v| v.find.as_ref()).filter(|_| this.can_find());
            let focus = field.map_or_else(|| this.focus.clone(), |find| find.field.read(cx).focus_handle(cx));
            window.focus(&focus);
        });
    }

    fn render_bar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let focused = self.query.read(cx).focus_handle(cx).is_focused(window);
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(12.))
            .py(px(10.))
            .border_b_1()
            .border_color(colours::border())
            .child(div().flex_1().min_w_0().child(ui::input_box(self.query.clone(), focused)))
            .child(ui::segments(vec![
                self.mode_segment("files-mode-names", "Name", Mode::Names, cx),
                self.mode_segment("files-mode-text", "Text", Mode::Text, cx),
            ]))
    }
}

impl Render for FilesPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.settle_focus(window, cx);
        let Some(place) = self.place() else {
            let note = self.no_place_note();
            return div()
                .size_full()
                .track_focus(&self.focus)
                .flex()
                .flex_col()
                .children(self.machine_strip())
                .child(ui::note(note))
                .into_any_element();
        };
        let viewing = place.viewer.is_some();
        let searching = !place.query.trim().is_empty();
        let body = if viewing {
            self.render_viewer(window, cx)
        } else if searching {
            self.render_results(cx)
        } else {
            self.render_tree(cx)
        };
        let menu = self.render_menu(cx);
        div()
            .size_full()
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .children(self.machine_strip())
            .when(!viewing, |d| d.child(self.render_bar(window, cx)))
            .child(body)
            .children(menu)
            .into_any_element()
    }
}

pub(crate) fn build(window: &mut Window, cx: &mut gpui::App) -> AnyView {
    cx.new(|cx| FilesPanel::new(window, cx)).into()
}
