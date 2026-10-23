//! The issue trackers: which are connected, their lists with the Issues dialog's filters, an
//! issue's detail, and starting work on it now or later (`cornercase issue sources|list|read|
//! people`, `start --issue`, `queue add --issue`). Connecting a tracker is Settings' job.

mod data;
mod markdown;
mod start;
mod text;

use std::collections::{HashMap, HashSet};
use std::process::Stdio;

use gpui::{
    AnyElement, AnyView, AppContext as _, ClickEvent, ClipboardItem, Context, Entity, Focusable as _, FontWeight,
    InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Window, div, prelude::FluentBuilder as _, px,
};

use crate::theme::FONT_FAMILY;
use crate::cli;
use crate::panels::PanelKind;
use crate::store::{Store, ToastKind};
use crate::theme::colours;
use crate::ui::{self, TextField, TextFieldEvent};

use data::{Agents, Issue, LIST_CAP, Listed, Lists, People, ReadIssue, SourceInfo, SourceState, Sources, Who};

/// A tab: every connected tracker together, or one.
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

/// One list as asked: its tab and filters, so going back to a filter shows its answer at once.
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
    /// The answer came from what the server held, a live one is on its way.
    cached: bool,
    loading: bool,
    error: Option<String>,
    loaded: bool,
}

type IssueId = (String, String);

/// The tabs a project had when a start began, to spot the one it makes.
struct Following {
    project: u64,
    known: HashSet<u64>,
    since: std::time::Instant,
}

pub(crate) struct IssuesPanel {
    /// The project asked: the one our view shows.
    project: Option<u64>,
    sources: Option<Sources>,
    sources_error: Option<String>,
    loading_sources: bool,
    tab: Tab,
    closed: bool,
    /// The assignee and author filters of each tab.
    people: HashMap<Tab, [Who; 2]>,
    lists: HashMap<ListKey, ListState>,
    selected: Option<Issue>,
    reads: HashMap<IssueId, Result<ReadIssue, String>>,
    reading: HashSet<IssueId>,
    raw: bool,
    search: Entity<TextField>,
    /// The open people picker: 0 for the assignee, 1 for the author.
    picker: Option<usize>,
    picker_search: Entity<TextField>,
    people_lists: HashMap<String, Result<People, String>>,
    agents: Option<Agents>,
    was_open: bool,
    /// A start whose tab is not in the session yet.
    following: Option<Following>,
    _subscriptions: Vec<Subscription>,
}

impl IssuesPanel {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| TextField::new("Search the list: words of the title, key, labels or people", cx));
        let picker_search = cx.new(|cx| TextField::new("Find someone", cx));
        let store = Store::get(cx);
        let subscriptions = vec![
            cx.subscribe_in(&search, window, |this, _, event, window, cx| match event {
                TextFieldEvent::Changed => cx.notify(),
                TextFieldEvent::Submit => this.open_start(false, window, cx),
                TextFieldEvent::Cancel => Store::get(cx).update(cx, Store::close_sheet),
            }),
            cx.subscribe_in(&picker_search, window, |this, _, event, _, cx| match event {
                TextFieldEvent::Changed | TextFieldEvent::Submit => cx.notify(),
                TextFieldEvent::Cancel => {
                    this.picker = None;
                    cx.notify();
                }
            }),
            // Opening the sheet, or showing another project, reads everything again.
            cx.observe_in(&store, window, |this, store, window, cx| {
                let (open, project) = {
                    let s = store.read(cx);
                    (s.sheet == Some(PanelKind::Issues), shown_project(s))
                };
                if open && (!this.was_open || project != this.project) {
                    this.project = project;
                    this.refresh(cx);
                    window.focus(&this.search.read(cx).focus_handle(cx));
                }
                this.was_open = open;
                this.follow_start(store.read(cx));
            }),
        ];
        let project = shown_project(store.read(cx));
        let mut panel = Self {
            project,
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
            raw: false,
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

    // -----------------------------------------------------------------------------------------
    // Asking cornercase.

    /// Reads the sources again, then the list shown (first what the server holds, then live),
    /// and the agents for the start form.
    fn refresh(&mut self, cx: &mut Context<Self>) {
        let Some(project) = self.project else { return };
        self.loading_sources = true;
        self.lists.clear();
        self.reads.clear();
        self.people_lists.clear();
        let first = self.sources.as_ref().is_none_or(|s| s.project != project);
        run_lenient(
            cx,
            args(&["issue", "sources", "--json", "--project", &project.to_string()]),
            move |this, out, cx| {
                if this.project != Some(project) {
                    return;
                }
                this.loading_sources = false;
                match out
                    .and_then(|out| data::parse::<Sources>(&out).ok_or_else(|| "cornercase answered no sources".into()))
                {
                    Ok(sources) => {
                        if first {
                            this.closed = sources.closed;
                            this.people.clear();
                            for source in &sources.sources {
                                this.people.insert(Tab::One(source.source.clone()), source.people.clone());
                            }
                            // Keep a tab that is still shown, else the first connected one when
                            // only one is, else All.
                            let connected: Vec<_> = sources.tabs().into_iter().filter(|s| s.connected()).collect();
                            let keep = this.tab.source().is_some_and(|s| sources.get(s).is_some_and(|s| s.tab));
                            if !keep {
                                this.tab = match connected.as_slice() {
                                    [one] => Tab::One(one.source.clone()),
                                    _ => Tab::All,
                                };
                            }
                        }
                        this.sources_error = None;
                        this.sources = Some(sources);
                        this.load_list(true, cx);
                    }
                    Err(e) => this.sources_error = Some(e),
                }
                cx.notify();
            },
        );
        if self.agents.is_none() {
            run_lenient(cx, args(&["config", "--json"]), |this, out, cx| {
                if let Ok(value) = out.and_then(|o| serde_json::from_str(&o).map_err(|e| e.to_string())) {
                    this.agents = Some(Agents::from_config(&value));
                    cx.notify();
                }
            });
        }
        cx.notify();
    }

    fn who(&self, tab: &Tab) -> [Who; 2] {
        self.people.get(tab).cloned().unwrap_or_default()
    }

    fn list_key(&self) -> Option<ListKey> {
        let people = self.who(&self.tab);
        Some(ListKey {
            project: self.project?,
            tab: self.tab.clone(),
            closed: self.closed,
            people: [people[0].arg(), people[1].arg()],
        })
    }

    /// Lists the tab with its filters; `cached` first answers from what the server holds and
    /// then asks the trackers.
    fn load_list(&mut self, cached: bool, cx: &mut Context<Self>) {
        let Some(key) = self.list_key() else { return };
        if self.tab.source().is_some_and(|s| !self.source(s).is_some_and(SourceInfo::connected)) {
            return;
        }
        if self.tab == Tab::All && !self.sources.as_ref().is_some_and(Sources::any_connected) {
            return;
        }
        let state = self.lists.entry(key.clone()).or_default();
        state.loading = true;
        let mut a = args(&["issue", "list", "--json", "--project", &key.project.to_string()]);
        if let Some(source) = key.tab.source() {
            a.extend(["--source".into(), source.into()]);
        }
        if key.closed {
            a.push("--closed".into());
        }
        // A person names one tracker's people, so the All tab only filters by me or anyone.
        let who = self.who(&key.tab);
        for (flag, who) in ["--assignee", "--author"].into_iter().zip(&who) {
            if *who != Who::Anyone && (key.tab != Tab::All || !who.is_person()) {
                a.extend([flag.into(), who.arg()]);
            }
        }
        if cached {
            a.push("--cached".into());
        }
        run_lenient(cx, a, move |this, out, cx| {
            let state = this.lists.entry(key.clone()).or_default();
            match out.and_then(|out| data::parse::<Lists>(&out).ok_or_else(|| first_line(&out))) {
                Ok(mut lists) => {
                    data::sort_newest_first(&mut lists.issues);
                    // A cached answer with nothing in it is not worth showing over "loading".
                    if !(cached && lists.issues.is_empty() && state.loaded) {
                        state.issues = lists.issues;
                        state.sources = lists.sources;
                    }
                    state.loaded = state.loaded || !cached || !state.issues.is_empty();
                    state.error = None;
                }
                Err(e) => {
                    if !cached {
                        state.error = Some(e);
                        state.loaded = true;
                    }
                }
            }
            state.cached = cached;
            state.loading = cached;
            if cached {
                this.load_list(false, cx);
            }
            cx.notify();
        });
        cx.notify();
    }

    fn source(&self, source: &str) -> Option<&SourceInfo> {
        self.sources.as_ref()?.get(source)
    }

    fn select(&mut self, issue: Issue, cx: &mut Context<Self>) {
        let id = issue.id();
        self.selected = Some(issue.clone());
        if !self.reads.contains_key(&id) && !self.reading.contains(&id) {
            self.read(issue, cx);
        }
        cx.notify();
    }

    fn read(&mut self, issue: Issue, cx: &mut Context<Self>) {
        let Some(project) = self.project else { return };
        let id = issue.id();
        self.reading.insert(id.clone());
        let a = args(&[
            "issue",
            "read",
            &issue.key,
            "--source",
            &issue.source,
            "--json",
            "--project",
            &project.to_string(),
        ]);
        run_lenient(cx, a, move |this, out, cx| {
            this.reading.remove(&id);
            let read = out.and_then(|out| data::parse::<ReadIssue>(&out).ok_or_else(|| first_line(&out)));
            this.reads.insert(id, read);
            cx.notify();
        });
    }

    fn load_people(&mut self, source: String, cx: &mut Context<Self>) {
        let Some(project) = self.project else { return };
        if self.people_lists.contains_key(&source) {
            return;
        }
        let a = args(&["issue", "people", &source, "--json", "--project", &project.to_string()]);
        run_lenient(cx, a, move |this, out, cx| {
            let people = out.and_then(|out| data::parse::<People>(&out).ok_or_else(|| first_line(&out)));
            this.people_lists.insert(source, people);
            cx.notify();
        });
    }

    // -----------------------------------------------------------------------------------------
    // Acting.

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

    /// Shows the list for the filters, asking for it unless it is known.
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

    /// The issues the list shows: the answer, narrowed by the search words.
    fn shown(&self, cx: &Context<Self>) -> Vec<Issue> {
        let query = self.search.read(cx).text();
        self.list_key()
            .and_then(|key| self.lists.get(&key))
            .map(|l| l.issues.iter().filter(|i| i.matches(&query)).cloned().collect())
            .unwrap_or_default()
    }

    fn move_selection(&mut self, by: isize, cx: &mut Context<Self>) {
        let shown = self.shown(cx);
        if shown.is_empty() {
            return;
        }
        let at = self.selected.as_ref().and_then(|s| shown.iter().position(|i| i.id() == s.id()));
        let next = match at {
            Some(at) => (at as isize + by).clamp(0, shown.len() as isize - 1) as usize,
            None => 0,
        };
        self.select(shown[next].clone(), cx);
    }

    /// Opens the start form for the selected issue, to start it now or `schedule` it.
    fn open_start(&mut self, schedule: bool, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(issue), Some(sources)) = (self.selected.clone(), self.sources.clone()) else { return };
        let repository = self.source(&issue.source).is_some_and(|s| s.repository);
        let agents = self.agents.clone().unwrap_or_default();
        let project_name = shown_project_name(cx);
        let panel = cx.entity().downgrade();
        start::open(start::Setup { issue, sources, repository, agents, schedule, project_name, panel }, window, cx);
    }

    /// Runs `start --issue`, then shows the agent's pane in our view.
    fn start(&mut self, plan: data::StartPlan, cx: &mut Context<Self>) {
        let (agent, key) = (plan.agent.clone(), plan.issue.key.clone());
        let store = Store::get(cx);
        // `start` answers once the agent took the prompt, which can wait on its folder trust
        // question: the new tab is shown as soon as it appears, so that question can be seen.
        if let Some(project) = plan.place.as_ref().map(|p| p.project) {
            let known = tab_ids(store.read(cx), project);
            self.following = Some(Following { project, known, since: std::time::Instant::now() });
        }
        store.update(cx, |s, cx| {
            s.toast(format!("Starting {agent} on {key}…"), ToastKind::Info, cx);
            s.close_sheet(cx);
        });
        run_lenient(cx, plan.start_args(), move |this, out, cx| {
            this.following = None;
            let store = Store::get(cx);
            store.update(cx, |s, cx| match out.as_deref().map(data::started_pane) {
                Ok(pane) => {
                    if let Some(pane) = pane {
                        s.focus("pane", pane);
                    }
                    s.toast(format!("{agent} is working on {key}"), ToastKind::Done, cx);
                }
                Err(e) => s.toast(format!("Could not start {key}: {e}"), ToastKind::Error, cx),
            });
        });
    }

    /// Shows the tab a start is making once the session has it.
    fn follow_start(&mut self, store: &Store) {
        let Some(following) = &self.following else { return };
        if following.since.elapsed() > std::time::Duration::from_secs(300) {
            self.following = None;
            return;
        }
        let new = tab_ids(store, following.project).into_iter().find(|t| !following.known.contains(t));
        if let Some(tab) = new {
            store.focus("tab", tab);
            self.following = None;
        }
    }

    /// Runs `queue add --issue`.
    fn schedule(&mut self, plan: data::StartPlan, at: String, cx: &mut Context<Self>) {
        let key = plan.issue.key.clone();
        run_lenient(cx, plan.queue_args(&at), move |_, out, cx| {
            Store::get(cx).update(cx, |s, cx| match out {
                Ok(id) => {
                    let id = id.trim();
                    s.toast(format!("{key} is scheduled for {at} (queue entry {id})"), ToastKind::Done, cx);
                }
                Err(e) => s.toast(format!("Could not schedule {key}: {e}"), ToastKind::Error, cx),
            });
        });
    }

    // -----------------------------------------------------------------------------------------
    // Drawing.

    fn render_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let mut items: Vec<AnyElement> = Vec::new();
        let any = self.sources.as_ref().is_some_and(Sources::any_connected);
        items.push(
            ui::segment("tab-all", "All", self.tab == Tab::All)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.set_tab(Tab::All, cx)))
                .when(!any, |d| d.text_color(colours::faint()))
                .into_any_element(),
        );
        for source in self.sources.as_ref().map(Sources::tabs).unwrap_or_default() {
            let tab = Tab::One(source.source.clone());
            let selected = self.tab == tab;
            let (dot, colour) = match source.state {
                SourceState::Connected => ("●", colours::done()),
                SourceState::Failing => ("!", colours::failed()),
                _ => ("○", colours::faint()),
            };
            items.push(
                ui::segment(SharedString::from(format!("tab-{}", source.source)), source.name.clone(), selected)
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .when(!source.connected() && !selected, |d| d.text_color(colours::faint()))
                    .child(div().text_size(px(8.)).text_color(colour).child(dot))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_tab(tab.clone(), cx)))
                    .into_any_element(),
            );
        }
        let loading = self.loading_sources || self.current().is_some_and(|l| l.loading);
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(ui::segments(items).flex_wrap())
            .child(div().flex_1())
            .when(loading, |d| d.child(ui::dim("loading…")))
            .child(
                ui::button("issues-refresh", "Refresh")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.refresh(cx))),
            )
    }

    fn current(&self) -> Option<&ListState> {
        self.lists.get(&self.list_key()?)
    }

    fn render_filters(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let focused = self.search.read(cx).focus_handle(cx).is_focused(window);
        let fields = match self.tab.source().and_then(|s| self.source(s)) {
            Some(source) => source.fields.clone(),
            None => ["assignee".to_string(), "author".to_string()],
        };
        let who = self.who(&self.tab);
        let mut row = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(div().flex_1().min_w(px(160.)).child(ui::input_box(self.search.clone(), focused)))
            .child(
                ui::checkbox("issues-closed", self.closed, "closed too")
                    .text_size(px(12.))
                    .text_color(colours::muted())
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.closed = !this.closed;
                        this.after_filter(cx);
                    })),
            );
        for (index, field) in fields.iter().enumerate() {
            let value = who[index].label();
            let active = who[index] != Who::Anyone;
            let open = self.picker == Some(index);
            row = row.child(
                ui::button_coloured(
                    ("issues-who", index),
                    format!("{}: {value} ▾", capitalise(field)),
                    if active { colours::accent() } else { colours::muted() },
                )
                .when(active || open, |d| d.border_color(colours::accent()))
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.toggle_picker(index, window, cx))),
            );
        }
        row
    }

    /// Who an assignee or author filter can name: anyone, me, and the tracker's people.
    fn render_picker(&self, index: usize, window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let current = self.who(&self.tab)[index].clone();
        let chip = |id: SharedString, label: String, selected: bool| {
            ui::segment(id, label, selected).border_1().border_color(if selected {
                colours::accent()
            } else {
                colours::border()
            })
        };
        let mut chips: Vec<AnyElement> = Vec::new();
        for who in [Who::Anyone, Who::Me] {
            let selected = current == who;
            chips.push(
                chip(SharedString::from(format!("who-{}", who.label())), who.label(), selected)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_who(index, who.clone(), cx)))
                    .into_any_element(),
            );
        }
        let mut note = None;
        match self.tab.source() {
            None => note = Some("Pick a tracker's tab to filter by someone in particular.".to_string()),
            Some(source) => match self.people_lists.get(source) {
                None => note = Some("Asking the tracker for its people…".into()),
                Some(Err(e)) => note = Some(e.clone()),
                Some(Ok(people)) => {
                    let query = self.picker_search.read(cx).text().to_lowercase();
                    let matching: Vec<_> = people
                        .people
                        .iter()
                        .filter(|p| query.is_empty() || p.label().to_lowercase().contains(&query))
                        .collect();
                    if matching.is_empty() {
                        note = Some("Nobody matches.".into());
                    }
                    for (i, person) in matching.into_iter().take(80).enumerate() {
                        let who = person.who();
                        let selected = current == who;
                        chips.push(
                            chip(SharedString::from(format!("who-p-{i}")), person.label(), selected)
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.set_who(index, who.clone(), cx);
                                }))
                                .into_any_element(),
                        );
                    }
                }
            },
        }
        let focused = self.picker_search.read(cx).focus_handle(cx).is_focused(window);
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .p(px(10.))
            .rounded(px(6.))
            .bg(colours::sidebar())
            .border_1()
            .border_color(colours::border())
            .when(self.tab.source().is_some(), |d| {
                d.child(div().w(px(260.)).child(ui::input_box(self.picker_search.clone(), focused)))
            })
            .child(
                div()
                    .id("who-chips")
                    .max_h(px(150.))
                    .overflow_y_scroll()
                    .flex()
                    .flex_wrap()
                    .gap(px(4.))
                    .children(chips),
            )
            .children(note.map(ui::dim))
    }

    fn render_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = self.current();
        let shown = self.shown(cx);
        let mut list = div().id("issues-list").size_full().overflow_y_scroll().flex().flex_col().gap(px(2.)).p(px(6.));
        // Trackers that failed in this list.
        let failures: Vec<&Listed> =
            state.map(|s| s.sources.iter().filter(|l| l.error.is_some()).collect()).unwrap_or_default();
        for failed in failures {
            let name = self.source(&failed.source).map_or(failed.source.clone(), |s| s.name.clone());
            list = list.child(error_note(format!("{name}: {}", failed.error.clone().unwrap_or_default())));
        }
        match state {
            None => return list.child(ui::note("Loading…")).into_any_element(),
            Some(state) if state.issues.is_empty() => {
                return list
                    .when_some(state.error.clone(), |d, e| d.child(error_note(e)))
                    .when(state.error.is_none(), |d| {
                        d.child(ui::note(if state.loading {
                            "Asking the trackers…"
                        } else {
                            "No issues match these filters."
                        }))
                    })
                    .into_any_element();
            }
            Some(state) => {
                if let Some(e) = &state.error {
                    list = list.child(error_note(e.clone()));
                }
            }
        }
        if shown.is_empty() {
            return list.child(ui::note("Nothing in this list matches the search.")).into_any_element();
        }
        let now = data::now();
        let all = self.tab == Tab::All;
        let selected = self.selected.as_ref().map(Issue::id);
        for (i, issue) in shown.into_iter().enumerate() {
            let is_selected = selected.as_ref() == Some(&issue.id());
            list = list.child(self.render_row(i, issue, is_selected, all, now, cx));
        }
        let count = state.map_or(0, |s| s.issues.len());
        if count >= LIST_CAP {
            list = list.child(
                ui::dim(format!(
                    "The first {LIST_CAP}, newest first: trackers answer no more. Narrow the filters to see others."
                ))
                .p(px(8.)),
            );
        }
        list.into_any_element()
    }

    fn render_row(
        &self,
        i: usize,
        issue: Issue,
        selected: bool,
        all: bool,
        now: i64,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let source = all.then(|| self.source(&issue.source).map_or(issue.source.clone(), |s| s.name.clone()));
        let mut meta: Vec<String> = Vec::new();
        if let Some(source) = source {
            meta.push(source);
        }
        if !issue.assignees.is_empty() {
            meta.push(format!("→ {}", issue.assignees.join(", ")));
        } else if !issue.author.is_empty() {
            meta.push(format!("by {}", issue.author));
        }
        if !issue.updated_at.is_empty() {
            meta.push(data::ago(&issue.updated_at, now));
        }
        let key_colour = if issue.closed { colours::faint() } else { colours::accent() };
        let labels = issue.labels.clone();
        let state = (!issue.state.is_empty() && issue.state != "open").then(|| issue.state.clone());
        div()
            .id(("issue-row", i))
            .flex()
            .flex_col()
            .gap(px(2.))
            .px(px(10.))
            .py(px(6.))
            .rounded(px(6.))
            .cursor_pointer()
            .when(selected, |d| d.bg(colours::selected()))
            .when(!selected, |d| d.hover(|s| s.bg(colours::hover())))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .flex_none()
                            .font_family(FONT_FAMILY)
                            .text_size(px(11.5))
                            .text_color(key_colour)
                            .child(issue.key.clone()),
                    )
                    .children(state.map(|s| state_chip(&s, issue.closed)))
                    .child(div().flex_1())
                    .children(labels.into_iter().take(3).map(label_chip)),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(if issue.closed { colours::muted() } else { colours::text() })
                    .line_clamp(2)
                    .child(issue.title.clone()),
            )
            .child(ui::dim(meta.join(" · ")).whitespace_nowrap().overflow_hidden().text_ellipsis())
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.select(issue.clone(), cx)))
    }

    fn render_detail(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(issue) = self.selected.clone() else {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(ui::note("Choose an issue to read it and start work on it."))
                .into_any_element();
        };
        let read = self.reads.get(&issue.id());
        let source = self.source(&issue.source).map_or(issue.source.clone(), |s| s.name.clone());
        let info: Vec<String> = match read {
            Some(Ok(read)) if !read.info.is_empty() => read.info.clone(),
            _ => {
                let mut info = vec![issue.state.clone()];
                if !issue.assignees.is_empty() {
                    info.push(format!("assigned to {}", issue.assignees.join(", ")));
                }
                if !issue.author.is_empty() {
                    info.push(format!("opened by {}", issue.author));
                }
                info
            }
        };
        let raw = self.raw;
        let url = issue.url.clone();
        let copy_url = issue.url.clone();
        let mut body = div().flex().flex_col().gap(px(8.)).text_size(px(13.)).text_color(colours::text());
        match read {
            None => body = body.child(ui::note("Reading the issue…").p_0()),
            Some(Err(e)) => body = body.child(error_note(e.clone())),
            Some(Ok(read)) => {
                if read.body.trim().is_empty() {
                    body = body.child(ui::dim("No description."));
                } else if raw {
                    body = body.child(text::raw(&read.body));
                } else {
                    body = body.children(text::markdown("body", &read.body));
                }
                if !read.comments.is_empty() {
                    body = body.child(ui::section(format!("Comments · {}", read.comments.len())));
                    let now = data::now();
                    for (i, comment) in read.comments.iter().enumerate() {
                        let when = if comment.created_at.is_empty() {
                            String::new()
                        } else {
                            data::ago(&comment.created_at, now)
                        };
                        body = body.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(6.))
                                .p(px(10.))
                                .rounded(px(6.))
                                .bg(colours::sidebar())
                                .border_1()
                                .border_color(colours::border())
                                .child(
                                    div()
                                        .flex()
                                        .gap(px(6.))
                                        .text_size(px(12.))
                                        .child(div().font_weight(FontWeight::SEMIBOLD).child(comment.author.clone()))
                                        .child(div().text_color(colours::faint()).child(when)),
                                )
                                .children(if raw {
                                    vec![text::raw(&comment.body)]
                                } else {
                                    text::markdown(&format!("comment-{i}"), &comment.body)
                                }),
                        );
                    }
                }
            }
        }
        let has_url = !url.is_empty();
        div()
            .id("issue-detail")
            .size_full()
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .p(px(18.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .text_size(px(12.))
                            .child(
                                div().font_family(FONT_FAMILY).text_color(colours::accent()).child(issue.key.clone()),
                            )
                            .child(div().text_color(colours::faint()).child(source))
                            .children((!issue.state.is_empty()).then(|| state_chip(&issue.state, issue.closed))),
                    )
                    .child(div().text_size(px(18.)).font_weight(FontWeight::SEMIBOLD).child(issue.title.clone()))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(6.))
                            .items_center()
                            .children(issue.info_extras(&info).into_iter().map(|i| ui::dim(i).text_size(px(12.))))
                            .children(issue.labels.iter().cloned().map(label_chip)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(6.))
                            .pb(px(6.))
                            .border_b_1()
                            .border_color(colours::border())
                            .child(ui::primary_button("issue-start", "Start…").on_click(
                                cx.listener(|this, _: &ClickEvent, window, cx| this.open_start(false, window, cx)),
                            ))
                            .child(ui::button("issue-schedule", "Schedule…").on_click(
                                cx.listener(|this, _: &ClickEvent, window, cx| this.open_start(true, window, cx)),
                            ))
                            .when(has_url, |d| {
                                d.child(
                                    ui::button("issue-open", "Open in browser")
                                        .on_click(move |_: &ClickEvent, _, cx| cx.open_url(&url)),
                                )
                                .child(ui::button("issue-copy", "Copy URL").on_click(
                                    move |_: &ClickEvent, _, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(copy_url.clone()));
                                        Store::get(cx)
                                            .update(cx, |s, cx| s.toast("Copied the issue's URL", ToastKind::Info, cx));
                                    },
                                ))
                            })
                            .child(div().flex_1())
                            .child(ui::segments(vec![
                                ui::segment("issue-rendered", "Rendered", !raw)
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.raw = false;
                                        cx.notify();
                                    }))
                                    .into_any_element(),
                                ui::segment("issue-raw", "Markdown", raw)
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.raw = true;
                                        cx.notify();
                                    }))
                                    .into_any_element(),
                            ])),
                    )
                    .child(body),
            )
            .into_any_element()
    }

    /// A tracker that is not connected (or none at all): what each needs, and the way to Settings.
    fn render_connect(&self, only: Option<&SourceInfo>, cx: &mut Context<Self>) -> AnyElement {
        let sources: Vec<SourceInfo> = match only {
            Some(source) => vec![source.clone()],
            None => self.sources.as_ref().map(|s| s.sources.clone()).unwrap_or_default(),
        };
        let (heading, how) = match only {
            Some(source) if source.source == "github" => (
                format!("{} is not connected", source.name),
                "GitHub goes through the gh command line in the project's folder: sign in with gh, or open a \
                 project whose repository is on GitHub."
                    .to_string(),
            ),
            Some(source) => (
                format!("{} is not connected", source.name),
                format!(
                    "Connect it in Settings → Issues → {}, or from a shell. Its token is checked and kept by \
                     cornercase, never by this window.",
                    source.name
                ),
            ),
            None => (
                "No issue tracker is connected".to_string(),
                "Connect GitHub with the gh command line, and the other trackers with a token in Settings → \
                 Issues. Tokens are checked and kept by cornercase, never by this window."
                    .to_string(),
            ),
        };
        let rows = sources.into_iter().map(|source| {
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .py(px(5.))
                .border_b_1()
                .border_color(colours::border())
                .child(div().w(px(80.)).flex_none().font_weight(FontWeight::SEMIBOLD).child(source.name.clone()))
                .child(div().w(px(260.)).flex_none().text_color(colours::muted()).child(source.missing_line()))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .font_family(FONT_FAMILY)
                        .text_size(px(11.))
                        .text_color(colours::faint())
                        .child(source.connect_hint()),
                )
        });
        div()
            .size_full()
            .flex()
            .justify_center()
            .pt(px(40.))
            .child(
                div()
                    .w(px(720.))
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(div().text_size(px(17.)).font_weight(FontWeight::SEMIBOLD).child(heading))
                    .child(div().text_color(colours::muted()).child(how))
                    .child(div().flex().flex_col().text_size(px(12.5)).children(rows))
                    .child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .pt(px(6.))
                            .child(ui::primary_button("issues-settings", "Open Settings").on_click(
                                |_: &ClickEvent, _, cx| {
                                    Store::get(cx).update(cx, |s, cx| s.open(PanelKind::Settings, cx));
                                },
                            ))
                            .child(
                                ui::button("issues-check", "Check again")
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.refresh(cx))),
                            ),
                    ),
            )
            .into_any_element()
    }
}

impl Render for IssuesPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root = div().size_full().flex().flex_col().on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
            match e.keystroke.key.as_str() {
                "down" => this.move_selection(1, cx),
                "up" => this.move_selection(-1, cx),
                _ => {}
            }
        }));
        if self.project.is_none() {
            return root.child(ui::note("Waiting for cornercase to show a project…"));
        }
        let Some(sources) = &self.sources else {
            return root.child(match &self.sources_error {
                Some(e) => error_note(e.clone()).m(px(16.)),
                None => ui::note("Asking cornercase which trackers are connected…"),
            });
        };
        if !sources.any_connected() {
            return root.child(self.render_connect(None, cx));
        }
        let header = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .px(px(16.))
            .pt(px(12.))
            .pb(px(10.))
            .border_b_1()
            .border_color(colours::border())
            .child(self.render_tabs(cx));
        let tab_source = self.tab.source().and_then(|s| self.source(s)).cloned();
        if let Some(source) = tab_source.as_ref().filter(|s| !s.connected()) {
            return root.child(header).child(self.render_connect(Some(source), cx));
        }
        let account = tab_source.as_ref().map(SourceInfo::account_line).filter(|a| !a.is_empty());
        let failing = tab_source.as_ref().filter(|s| s.state == SourceState::Failing).and_then(|s| s.reason.clone());
        let header = header
            .child(self.render_filters(window, cx))
            .children(self.picker.map(|index| self.render_picker(index, window, cx)))
            .children(failing.map(error_note))
            .children(account.map(ui::dim));
        root.child(header).child(
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
                .child(div().flex_1().min_w_0().h_full().child(self.render_detail(cx))),
        )
    }
}

pub(crate) fn build(window: &mut Window, cx: &mut gpui::App) -> AnyView {
    cx.new(|cx| IssuesPanel::new(window, cx)).into()
}

// ---------------------------------------------------------------------------------------------

fn args(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| (*s).to_string()).collect()
}

fn shown_project(store: &Store) -> Option<u64> {
    store.session.as_ref().and_then(|s| s.shown().project)
}

/// Every tab of a project, in all its workspaces.
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

fn shown_project_name(cx: &gpui::App) -> String {
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

/// Runs a cornercase command off the main thread and hands back its standard output when it
/// printed any, even with a failing status: `issue list` prints the trackers that answered and
/// exits 1 when one failed. Otherwise the error is what it wrote on stderr.
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

fn error_note(text: String) -> gpui::Div {
    div()
        .px(px(10.))
        .py(px(6.))
        .rounded(px(5.))
        .text_size(px(12.))
        .text_color(colours::failed())
        .bg(gpui::rgba(0xf7768e14))
        .child(text)
}

fn label_chip(label: String) -> gpui::Div {
    div()
        .flex_none()
        .px(px(6.))
        .rounded(px(9.))
        .text_size(px(10.5))
        .text_color(colours::muted())
        .border_1()
        .border_color(colours::border())
        .child(label)
}

fn state_chip(state: &str, closed: bool) -> gpui::Div {
    let colour = if closed { colours::faint() } else { colours::done() };
    div()
        .flex_none()
        .px(px(6.))
        .rounded(px(9.))
        .text_size(px(10.5))
        .text_color(colour)
        .border_1()
        .border_color(colour)
        .child(state.to_string())
}
