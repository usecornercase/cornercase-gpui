mod data;
mod detail;
mod list;
mod load;
mod markdown;
mod start;
mod text;

use std::collections::{HashMap, HashSet};
use std::process::Stdio;
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, AnyView, App, AppContext as _, Context, Div, Entity, Focusable as _, InteractiveElement as _,
    IntoElement, KeyDownEvent, ParentElement as _, Render, Styled as _, Subscription, Window, div, px,
};

use crate::cli;
use crate::panels::PanelKind;
use crate::store::{Store, ToastKind};
use crate::theme::colours;
use crate::ui::{self, TextField, TextFieldEvent};

use data::{Agents, Issue, Listed, ReadIssue, SourceInfo, SourceState, Sources, StartPlan, Who};

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
    reads: HashMap<IssueId, Result<ReadIssue, String>>,
    reading: HashSet<IssueId>,
    body_view: BodyView,
    search: Entity<TextField>,
    picker: Option<usize>,
    picker_search: Entity<TextField>,
    people_lists: HashMap<String, Result<data::People, String>>,
    agents: Option<Agents>,
    was_open: bool,
    following: Option<Following>,
    _subscriptions: Vec<Subscription>,
}

impl IssuesPanel {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| TextField::new("Search the list: words of the title, key, labels or people", cx));
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
                TextFieldEvent::Changed => cx.notify(),
                TextFieldEvent::Submit => this.open_start(false, window, cx),
                TextFieldEvent::Cancel => Store::get(cx).update(cx, Store::close_sheet),
            }),
            cx.subscribe_in(picker_search, window, |this, _, event, _, cx| match event {
                TextFieldEvent::Changed | TextFieldEvent::Submit => cx.notify(),
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
            self.project = project;
            self.refresh(cx);
            window.focus(&self.search.read(cx).focus_handle(cx));
        }
        self.was_open = open;
        self.follow_start(store.read(cx));
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
        cx.notify();
    }

    fn toggle_picker(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.picker = if self.picker == Some(index) { None } else { Some(index) };
        if self.picker.is_some() {
            self.picker_search.update(cx, TextField::clear);
            window.focus(&self.picker_search.read(cx).focus_handle(cx));
            if let Some(source) = self.tab.source() {
                self.load_people(source.to_string(), cx);
            }
        }
        cx.notify();
    }

    fn close_picker(&mut self, cx: &mut Context<Self>) {
        self.picker = None;
        cx.notify();
    }

    fn on_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "down" => self.move_selection(true, cx),
            "up" => self.move_selection(false, cx),
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

    fn start(&mut self, plan: &StartPlan, cx: &mut Context<Self>) {
        let (agent, key) = (plan.agent.clone(), plan.issue.key.clone());
        let store = Store::get(cx);
        if let Some(project) = plan.place.as_ref().map(|p| p.project) {
            let known = tab_ids(store.read(cx), project);
            self.following = Some(Following { project, known, since: Instant::now() });
        }
        store.update(cx, |s, cx| {
            s.toast(format!("Starting {agent} on {key}…"), ToastKind::Info, cx);
            s.close_sheet(cx);
        });
        run_lenient(cx, plan.start_args(), move |this, out, cx| {
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
        run_lenient(cx, plan.queue_args(&at), move |_, out, cx| {
            let (text, kind) = match out {
                Ok(id) => (format!("{key} is scheduled for {at} (queue entry {})", id.trim()), ToastKind::Done),
                Err(e) => (format!("Could not schedule {key}: {e}"), ToastKind::Error),
            };
            Store::get(cx).update(cx, |s, cx| s.toast(text, kind, cx));
        });
    }

    fn content(&self, window: &Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        if self.project.is_none() {
            return vec![ui::note("Waiting for cornercase to show a project…").into_any_element()];
        }
        let Some(sources) = &self.sources else {
            return vec![self.sources_note().into_any_element()];
        };
        if !sources.any_connected() {
            return vec![self.render_connect(None, cx)];
        }
        let tab_source = self.tab.source().and_then(|s| self.source(s)).cloned();
        if let Some(source) = tab_source.as_ref().filter(|s| !s.connected()) {
            return vec![self.header(cx).into_any_element(), self.render_connect(Some(source), cx)];
        }
        vec![
            self.header_with_filters(tab_source.as_ref(), window, cx).into_any_element(),
            self.panes(cx).into_any_element(),
        ]
    }

    fn sources_note(&self) -> Div {
        match &self.sources_error {
            Some(e) => error_note(e.clone()).m(px(16.)),
            None => ui::note("Asking cornercase which trackers are connected…"),
        }
    }

    fn header(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .px(px(16.))
            .pt(px(12.))
            .pb(px(10.))
            .border_b_1()
            .border_color(colours::border())
            .child(self.render_tabs(cx))
    }

    fn header_with_filters(&self, source: Option<&SourceInfo>, window: &Window, cx: &mut Context<Self>) -> Div {
        let account = source.map(SourceInfo::account_line).filter(|a| !a.is_empty());
        let failing = source.filter(|s| s.state == SourceState::Failing).and_then(|s| s.reason.clone());
        self.header(cx)
            .child(self.render_filters(window, cx))
            .children(self.picker.map(|index| self.render_picker(index, window, cx)))
            .children(failing.map(error_note))
            .children(account.map(ui::dim))
    }

    fn panes(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .child(
                div()
                    .w(px(400.))
                    .flex_none()
                    .h_full()
                    .border_r_1()
                    .border_color(colours::border())
                    .child(self.render_list(cx)),
            )
            .child(div().flex_1().min_w_0().h_full().child(self.render_detail(cx)))
    }
}

impl Render for IssuesPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| this.on_key(event, cx)))
            .children(self.content(window, cx))
    }
}

pub(crate) fn build(window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|cx| IssuesPanel::new(window, cx)).into()
}

fn started(out: &Result<String, String>, agent: &str, key: &str, cx: &mut App) {
    Store::get(cx).update(cx, |s, cx| match out.as_deref().map(data::started_pane) {
        Ok(pane) => {
            if let Some(pane) = pane {
                s.focus("pane", pane);
            }
            s.toast(format!("{agent} is working on {key}"), ToastKind::Done, cx);
        }
        Err(e) => s.toast(format!("Could not start {key}: {e}"), ToastKind::Error, cx),
    });
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

fn run_lenient(
    cx: &mut Context<IssuesPanel>,
    args: Vec<String>,
    then: impl FnOnce(&mut IssuesPanel, Result<String, String>, &mut Context<IssuesPanel>) + 'static,
) {
    let task = cx.background_spawn(async move {
        let out =
            cli::command(&args).stdin(Stdio::null()).output().map_err(|e| format!("could not run cornercase: {e}"))?;
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        if out.status.success() || stdout.trim_start().starts_with('{') {
            return Ok(stdout);
        }
        let err = String::from_utf8_lossy(&out.stderr);
        let err = err.trim().trim_start_matches("Error: ");
        Err(if err.is_empty() { "cornercase failed".to_string() } else { err.to_string() })
    });
    cx.spawn(async move |this, cx| {
        let result = task.await;
        let _ = this.update(cx, |this, cx| then(this, result, cx));
    })
    .detach();
}

fn first_line(text: &str) -> String {
    let line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("cornercase answered nothing");
    line.trim().to_string()
}

fn capitalise(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

fn error_note(text: String) -> Div {
    div()
        .px(px(10.))
        .py(px(6.))
        .rounded(px(5.))
        .text_size(px(12.))
        .text_color(colours::failed())
        .bg(gpui::rgba(0xf776_8e14))
        .child(text)
}

fn chip(text: String, colour: gpui::Rgba, border: gpui::Rgba) -> Div {
    div()
        .flex_none()
        .px(px(6.))
        .rounded(px(9.))
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
    let colour = if closed { colours::faint() } else { colours::done() };
    chip(state.to_string(), colour, colour)
}
