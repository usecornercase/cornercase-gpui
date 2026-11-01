mod connect;
mod data;
mod detail;
mod list;
mod load;
mod markdown;
mod page;
mod picker;
mod start;
mod text;
mod toolbar;

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, AnyView, App, AppContext as _, ClipboardItem, Context, Div, Entity, Focusable as _,
    InteractiveElement as _, IntoElement, KeyDownEvent, ListAlignment, MouseButton, MouseDownEvent, ParentElement as _,
    Render, SharedString, Styled as _, Subscription, Svg, Window, div, px,
};

use crate::panels::{PanelKind, flows};
use crate::store::{self, Busy, Store, ToastKind};
use crate::theme::{colours, radius};
use crate::ui::split::{self, Edge, Split};
use crate::ui::{self, Icon, TextField, TextFieldEvent, glass};

use data::{Agents, Issue, Listed, SourceState, Sources, StartPlan, Who};
use list::Rows;
use page::Read;

const LIST: Split = Split::new(400., 260., 800.);
const DETAIL_KEEP: f32 = 320.;
const OVERDRAW: f32 = 400.;

struct ListGrip;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Tab {
    All,
    One(String),
}

impl Tab {
    fn source(&self) -> Option<&str> {
        match self {
            Self::All => None,
            Self::One(source) => Some(source),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ListKey {
    project: u64,
    tab: Tab,
    closed: bool,
    people: [String; 2],
}

#[derive(Default)]
struct ListState {
    issues: Vec<Issue>,
    sources: Vec<Listed>,
    cached: bool,
    loading: bool,
    error: Option<String>,
    loaded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BodyView {
    Rendered,
    Markdown,
}

type IssueId = (String, String);
type Asked = (u64, IssueId);

struct Following {
    project: u64,
    known: HashSet<u64>,
    since: Instant,
}

pub(crate) struct IssuesPanel {
    project: Option<u64>,
    sources: Option<Sources>,
    sources_error: Option<String>,
    loading_sources: bool,
    tab: Tab,
    closed: bool,
    people: HashMap<Tab, [Who; 2]>,
    lists: HashMap<ListKey, ListState>,
    selected: Option<Issue>,
    reads: HashMap<IssueId, Read>,
    reading: HashSet<Asked>,
    body_view: BodyView,
    search: Entity<TextField>,
    picker: Option<usize>,
    picker_search: Entity<TextField>,
    people_lists: HashMap<String, Result<data::People, String>>,
    agents: Option<Agents>,
    was_open: bool,
    following: Option<Following>,
    list_width: Split,
    rows: Rows,
    _subscriptions: Vec<Subscription>,
}

impl IssuesPanel {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| TextField::new("Filter by title, key, label or person", cx).clears_on_escape());
        let picker_search = cx.new(|cx| TextField::new("Find someone", cx));
        let subscriptions = Self::subscriptions(&search, &picker_search, window, cx);
        let mut panel = Self {
            project: shown_project(Store::get(cx).read(cx)),
            sources: None,
            sources_error: None,
            loading_sources: false,
            tab: Tab::All,
            closed: false,
            people: HashMap::new(),
            lists: HashMap::new(),
            selected: None,
            reads: HashMap::new(),
            reading: HashSet::new(),
            body_view: BodyView::Rendered,
            search,
            picker: None,
            picker_search,
            people_lists: HashMap::new(),
            agents: None,
            was_open: true,
            following: None,
            list_width: LIST,
            rows: Rows::new(),
            _subscriptions: subscriptions,
        };
        panel.refresh(cx);
        window.focus(&panel.search.read(cx).focus_handle(cx));
        panel
    }

    fn subscriptions(
        search: &Entity<TextField>,
        picker_search: &Entity<TextField>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Subscription> {
        let store = Store::get(cx);
        vec![
            cx.subscribe_in(search, window, |this, _, event, window, cx| match event {
                TextFieldEvent::Changed => this.search_changed(cx),
                TextFieldEvent::Cancel => cx.notify(),
                TextFieldEvent::Submit => this.open_start(false, window, cx),
            }),
            cx.subscribe_in(picker_search, window, |this, _, event, _, cx| match event {
                TextFieldEvent::Changed => cx.notify(),
                TextFieldEvent::Submit => this.pick_first(cx),
                TextFieldEvent::Cancel => this.close_picker(cx),
            }),
            cx.observe_in(&store, window, |this, store, window, cx| this.store_changed(&store, window, cx)),
        ]
    }

    fn store_changed(&mut self, store: &Entity<Store>, window: &mut Window, cx: &mut Context<Self>) {
        let (open, project) = {
            let s = store.read(cx);
            (s.sheet == Some(PanelKind::Issues), shown_project(s))
        };
        if open && (!self.was_open || project != self.project) {
            self.show_project(project, cx);
            window.focus(&self.search.read(cx).focus_handle(cx));
        }
        if !open {
            self.close_picker(cx);
        }
        self.was_open = open;
        self.follow_start(store.read(cx));
    }

    fn show_project(&mut self, project: Option<u64>, cx: &mut Context<Self>) {
        if project != self.project {
            self.selected = None;
        }
        self.project = project;
        self.refresh(cx);
    }

    fn set_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        if self.tab != tab {
            self.tab = tab;
            self.picker = None;
            self.after_filter(cx);
        }
    }

    fn set_who(&mut self, index: usize, who: Who, cx: &mut Context<Self>) {
        let mut people = self.who(&self.tab);
        people[index] = who;
        self.people.insert(self.tab.clone(), people);
        self.picker = None;
        self.after_filter(cx);
    }

    fn toggle_closed(&mut self, cx: &mut Context<Self>) {
        self.closed = !self.closed;
        self.after_filter(cx);
    }

    fn set_body_view(&mut self, view: BodyView, cx: &mut Context<Self>) {
        self.body_view = view;
        cx.notify();
    }

    fn after_filter(&mut self, cx: &mut Context<Self>) {
        if let Some(key) = self.list_key()
            && !self.lists.get(&key).is_some_and(|l| l.loaded || l.loading)
        {
            self.load_list(true, cx);
        }
        self.scroll_list_to_top();
        cx.notify();
    }

    fn search_changed(&mut self, cx: &mut Context<Self>) {
        self.scroll_list_to_top();
        cx.notify();
    }

    fn clear_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search.update(cx, TextField::clear);
        window.focus(&self.search.read(cx).focus_handle(cx));
    }

    fn scroll_list_to_top(&self) {
        self.rows.to_top();
    }

    fn sync_rows(&mut self, cx: &Context<Self>) {
        let capped = self.current().is_some_and(|s| s.issues.len() >= data::LIST_CAP);
        let shown = self.shown(cx);
        self.rows.sync(shown, capped, self.list_width.size());
    }

    fn toggle_picker(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.picker = if self.picker == Some(index) { None } else { Some(index) };
        if self.picker.is_some()
            && let Some(source) = self.tab.source()
        {
            self.picker_search.update(cx, TextField::clear);
            window.focus(&self.picker_search.read(cx).focus_handle(cx));
            self.load_people(source.to_string(), cx);
        }
        cx.notify();
    }

    fn close_picker(&mut self, cx: &mut Context<Self>) {
        if self.picker.take().is_some() {
            cx.notify();
        }
    }

    fn pick_first(&mut self, cx: &mut Context<Self>) {
        let (Some(index), Some(source)) = (self.picker, self.tab.source()) else { return };
        let query = self.picker_search.read(cx).text();
        let Some(Ok(people)) = self.people_lists.get(source) else { return };
        let first = people.matching(&query).first().map(|p| p.who()).filter(|_| !query.trim().is_empty());
        if let Some(who) = first {
            self.set_who(index, who, cx);
        }
    }

    fn on_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "down" => self.move_selection(true, cx),
            "up" => self.move_selection(false, cx),
            "escape" if self.picker.is_some() => {
                self.close_picker(cx);
                cx.stop_propagation();
            }
            _ => {}
        }
    }

    fn move_selection(&mut self, down: bool, cx: &mut Context<Self>) {
        let shown = self.shown(cx);
        let Some(last) = shown.len().checked_sub(1) else { return };
        let at = self.selected.as_ref().and_then(|s| shown.iter().position(|i| i.id() == s.id()));
        let next = match at {
            Some(at) if down => (at + 1).min(last),
            Some(at) => at.saturating_sub(1),
            None => 0,
        };
        self.rows.reveal(next);
        self.select(shown[next].clone(), cx);
    }

    fn open_start(&mut self, schedule: bool, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(issue), Some(sources)) = (self.selected.clone(), self.sources.clone()) else { return };
        let repository = self.source(&issue.source).is_some_and(|s| s.repository);
        let agents = self.agents.clone().unwrap_or_default();
        let project_name = shown_project_name(cx);
        let panel = cx.entity().downgrade();
        start::open(start::Setup { issue, sources, repository, agents, schedule, project_name, panel }, window, cx);
    }

    fn start_flow(&self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(issue), Some(sources)) = (&self.selected, &self.sources) else { return };
        let repository = self.source(&issue.source).is_some_and(|s| s.repository);
        let read = self.reads.get(&issue.id()).and_then(|r| r.as_ref().ok()).map(|page| &page.read);
        flows::start_flow(data::flow_start(issue, sources, repository, read), window, cx);
    }

    fn start(&mut self, plan: &StartPlan, cx: &mut Context<Self>) {
        let (agent, key) = (plan.agent.clone(), plan.issue.key.clone());
        let store = Store::get(cx);
        if let Some(project) = plan.place.as_ref().map(|p| p.project) {
            let known = tab_ids(store.read(cx), project);
            self.following = Some(Following { project, known, since: Instant::now() });
        }
        let busy = Busy::new(format!("issue-start:{key}"), format!("Starting {agent} on {key}…"));
        let (command, job) = store.update(cx, |s, cx| {
            s.close_sheet(cx);
            (s.with_view(plan.start_args()), s.start_job(busy, cx))
        });
        ask(cx, command, move |this, out, cx| {
            Store::get(cx).update(cx, |s, cx| s.end_job(job, cx));
            this.following = None;
            started(&out, &agent, &key, cx);
        });
    }

    fn follow_start(&mut self, store: &Store) {
        let Some(following) = &self.following else { return };
        if following.since.elapsed() > Duration::from_secs(300) {
            self.following = None;
            return;
        }
        let new = tab_ids(store, following.project).into_iter().find(|t| !following.known.contains(t));
        if let Some(tab) = new {
            store.focus("tab", tab);
            self.following = None;
        }
    }

    fn schedule(plan: &StartPlan, at: String, cx: &mut Context<Self>) {
        let key = plan.issue.key.clone();
        ask(cx, plan.queue_args(&at), move |_, out, cx| {
            let (text, kind) = match out {
                Ok(id) => (format!("{key} is scheduled for {at} (queue entry {})", id.trim()), ToastKind::Done),
                Err(e) => (format!("Could not schedule {key}: {e}"), ToastKind::Error),
            };
            Store::get(cx).update(cx, |s, cx| s.toast(text, kind, cx));
        });
    }

    fn content(&self, window: &Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        if self.project.is_none() {
            return vec![ui::loading("Waiting for cornercase to show a project…").into_any_element()];
        }
        let Some(sources) = &self.sources else {
            return vec![self.sources_note().into_any_element()];
        };
        if !sources.any_connected() {
            return vec![self.render_connect(None, cx)];
        }
        let tab_source = self.tab.source().and_then(|s| self.source(s)).cloned();
        if let Some(source) = tab_source.as_ref().filter(|s| !s.connected()) {
            return vec![self.toolbar(cx).into_any_element(), self.render_connect(Some(source), cx)];
        }
        vec![
            self.toolbar_with_filters(tab_source.as_ref(), window, cx).into_any_element(),
            self.panes(cx).into_any_element(),
        ]
    }

    fn sources_note(&self) -> Div {
        match &self.sources_error {
            Some(e) => error_note(e.clone()).m(px(16.)),
            None => ui::loading("Asking cornercase which trackers are connected…"),
        }
    }

    fn panes(&self, cx: &mut Context<Self>) -> Div {
        let width = self.list_width.size();
        div()
            .flex_1()
            .min_h_0()
            .relative()
            .flex()
            .on_drag_move(split::follow::<_, ListGrip>(Edge::Right, DETAIL_KEEP, list_width, cx))
            .child(
                div()
                    .w(px(width))
                    .flex_none()
                    .h_full()
                    .border_r_1()
                    .border_color(colours::border())
                    .child(self.render_list(cx)),
            )
            .child(div().flex_1().min_w_0().h_full().child(self.render_detail(cx)))
            .child(split::handle("issues-split", ListGrip, (Edge::Right, width), list_width, cx))
    }
}

fn list_width(panel: &mut IssuesPanel) -> &mut Split {
    &mut panel.list_width
}

impl Render for IssuesPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_rows(cx);
        div()
            .size_full()
            .flex()
            .flex_col()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| this.on_key(event, cx)))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _: &MouseDownEvent, _, cx| this.close_picker(cx)))
            .children(self.content(window, cx))
    }
}

pub(crate) fn build(window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|cx| IssuesPanel::new(window, cx)).into()
}

fn virtual_list(count: usize) -> gpui::ListState {
    gpui::ListState::new(count, ListAlignment::Top, px(OVERDRAW))
}

fn started(out: &Result<String, String>, agent: &str, key: &str, cx: &mut App) {
    let (text, kind) = match out {
        Ok(_) => (format!("{agent} is working on {key}"), ToastKind::Done),
        Err(e) => (format!("Could not start {key}: {e}"), ToastKind::Error),
    };
    Store::get(cx).update(cx, |s, cx| s.toast(text, kind, cx));
}

fn shown_project(store: &Store) -> Option<u64> {
    store.session.as_ref().and_then(|s| s.shown().project)
}

fn tab_ids(store: &Store, project: u64) -> HashSet<u64> {
    let projects = store.session.as_ref().map(|s| s.projects.as_slice()).unwrap_or_default();
    projects
        .iter()
        .filter(|p| p.id == project)
        .flat_map(|p| &p.workspaces)
        .flat_map(|w| &w.tabs)
        .map(|t| t.id)
        .collect()
}

fn shown_project_name(cx: &App) -> String {
    let store = Store::get(cx);
    let s = store.read(cx);
    s.session
        .as_ref()
        .and_then(|session| {
            let id = session.shown().project?;
            session.projects.iter().find(|p| p.id == id).map(|p| p.name.clone())
        })
        .unwrap_or_default()
}

fn ask(
    cx: &mut Context<IssuesPanel>,
    args: Vec<String>,
    then: impl FnOnce(&mut IssuesPanel, Result<String, String>, &mut Context<IssuesPanel>) + 'static,
) {
    store::run_partial(cx, args, move |this, out, cx| then(this, out.map_err(|e| e.to_string()), cx));
}

fn first_line(text: &str) -> String {
    let line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("cornercase answered nothing");
    line.trim().to_string()
}

fn copy_text(text: String, what: &str, cx: &mut App) {
    cx.write_to_clipboard(ClipboardItem::new_string(text));
    Store::get(cx).update(cx, |s, cx| s.toast(format!("Copied {what}"), ToastKind::Info, cx));
}

fn error_note(text: String) -> Div {
    div()
        .px(px(12.))
        .py(px(8.))
        .rounded(px(radius::CONTROL))
        .text_size(px(12.))
        .text_color(colours::failed())
        .bg(glass::tint(colours::failed(), 0.08))
        .child(text)
}

fn card() -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .p(px(12.))
        .rounded(px(radius::CARD))
        .bg(colours::panel())
        .border_1()
        .border_color(colours::border())
}

fn blank(text: impl Into<SharedString>) -> Div {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(10.))
        .p(px(24.))
        .child(ui::icon(Icon::Issues, 26.).text_color(colours::faint()))
        .child(div().max_w(px(280.)).text_center().text_size(px(12.5)).text_color(colours::muted()).child(text.into()))
}

fn state_icon(issue: &Issue, size: f32) -> Svg {
    if issue.closed {
        ui::icon(Icon::Closed, size).text_color(colours::faint())
    } else {
        ui::icon(Icon::Issues, size).text_color(colours::done())
    }
}

fn state_dot(state: SourceState) -> Div {
    let dot = div().size(px(6.)).flex_none().rounded_full();
    match state {
        SourceState::Connected => dot.bg(colours::done()),
        SourceState::Failing => dot.bg(colours::failed()),
        _ => dot.border_1().border_color(colours::faint()),
    }
}

fn chip(text: String, colour: gpui::Rgba, border: gpui::Rgba) -> Div {
    div()
        .flex_none()
        .px(px(7.))
        .rounded(px(radius::CHIP))
        .text_size(px(10.5))
        .text_color(colour)
        .border_1()
        .border_color(border)
        .child(text)
}

fn label_chip(label: String) -> Div {
    chip(label, colours::muted(), colours::border())
}

fn state_chip(state: &str, closed: bool) -> Div {
    let colour = if closed { colours::faint() } else { colours::accent() };
    chip(state.to_string(), colour, glass::tint(colour, 0.45))
}
