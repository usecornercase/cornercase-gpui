mod actions;
mod diagram;
mod dialog;
mod flow_page;
mod import;
mod list;
pub(crate) mod model;
mod run_page;
mod start;
mod widgets;

use std::collections::HashSet;
use std::time::Duration;

use gpui::{
    AnyElement, AnyView, AppContext as _, AsyncApp, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, StatefulInteractiveElement as _, Styled as _, Task, WeakEntity, Window, div, px,
};

use self::model::{Files, FlowInfo, Frame, Listing, Message, Run};
use crate::cli::Stream;
use crate::panels::PanelKind;
use crate::store::{self, Store, args};
use crate::ui;

const POLL: Duration = Duration::from_secs(3);
const KEPT: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Selected {
    Run(u64),
    Flow(String),
    Hidden(String),
    Broken(String),
}

#[derive(Default)]
struct Feed {
    key: Option<(u64, u64)>,
    messages: Vec<Message>,
    ended: Option<String>,
    stream: Option<Stream>,
}

pub(crate) struct FlowsPanel {
    project: Option<u64>,
    listing: Option<Listing>,
    list_error: Option<String>,
    listing_busy: bool,
    runs: Vec<Run>,
    runs_error: Option<String>,
    runs_stream: Option<Stream>,
    selected: Option<Selected>,
    feed: Feed,
    files: Option<Result<Files, String>>,
    expanded: HashSet<u64>,
    prompt: Option<String>,
}

impl FlowsPanel {
    pub(crate) fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = Store::get(cx);
        cx.observe(&store, |_, _, cx| cx.notify()).detach();
        let mut this = Self {
            project: None,
            listing: None,
            list_error: None,
            listing_busy: false,
            runs: Vec::new(),
            runs_error: None,
            runs_stream: None,
            selected: None,
            feed: Feed::default(),
            files: None,
            expanded: HashSet::new(),
            prompt: None,
        };
        this.follow_runs(cx);
        this.refresh(cx);
        cx.spawn(async move |this, cx| poll_while_alive(this, cx).await).detach();
        this
    }

    fn poll(&mut self, cx: &mut Context<Self>) {
        if Store::get(cx).read(cx).sheet == Some(PanelKind::Flows) {
            self.refresh(cx);
            self.read_files(cx);
        }
    }

    pub(crate) fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.listing_busy {
            return;
        }
        self.listing_busy = true;
        let project = Store::get(cx).read(cx).session.as_ref().and_then(|s| s.shown().project);
        let mut command = args(&["flow", "list", "--json"]);
        command.extend(model::project_args(project));
        store::run(cx, command, move |this, result, cx| {
            this.listing_busy = false;
            this.project = project;
            this.apply_listing(result.and_then(|out| model::parse_listing(&out)));
            cx.notify();
        });
    }

    fn apply_listing(&mut self, result: anyhow::Result<Listing>) {
        match result {
            Ok(listing) => {
                if self.listing.as_ref() != Some(&listing) {
                    self.listing = Some(listing);
                    self.ensure_selection();
                }
                self.list_error = None;
            }
            Err(e) => self.list_error = Some(e.to_string()),
        }
    }

    fn follow_runs(&mut self, cx: &mut Context<Self>) {
        if self.runs_stream.is_some() {
            return;
        }
        self.runs_stream = store::follow_until(
            cx,
            args(&["flow", "runs", "--follow"]),
            |this, line, cx| this.on_runs_line(&line, cx),
            |this, err, cx| this.on_runs_end(&err, cx),
        );
    }

    fn on_runs_line(&mut self, line: &str, cx: &mut Context<Self>) {
        match model::parse_runs(line) {
            Ok(runs) => {
                self.runs = runs.runs;
                self.runs_error = None;
                self.ensure_selection();
                self.sync_feed(cx);
                cx.notify();
            }
            Err(e) => self.runs_error = Some(format!("cannot read `flow runs`: {e}")),
        }
    }

    fn on_runs_end(&mut self, err: &str, cx: &mut Context<Self>) {
        self.runs_error = Some(ended_because(err, "stopped following the runs"));
        self.runs_stream = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(POLL).await;
            let _ = this.update(cx, Self::follow_runs);
        })
        .detach();
    }

    fn sync_feed(&mut self, cx: &mut Context<Self>) {
        let key = self.selected_run().filter(|r| !r.closed()).map(|r| (r.id, r.workspace));
        if key == self.feed.key && (key.is_none() || self.feed.stream.is_some() || self.feed.ended.is_some()) {
            return;
        }
        let Some(followed) = key else {
            self.feed = Feed::default();
            return;
        };
        let keep = self.feed.key == key;
        let messages = if keep { std::mem::take(&mut self.feed.messages) } else { Vec::new() };
        if !keep {
            self.expanded.clear();
        }
        self.feed = Feed { key, messages, ended: None, stream: None };
        let workspace = followed.1.to_string();
        self.feed.stream = store::follow_until(
            cx,
            args(&["flow", "messages", "--workspace", &workspace, "--follow", "--json"]),
            move |this, line, cx| this.on_feed_line(followed, &line, cx),
            move |this, err, cx| this.on_feed_end(followed, &err, cx),
        );
        self.read_files(cx);
    }

    fn on_feed_line(&mut self, followed: (u64, u64), line: &str, cx: &mut Context<Self>) {
        if self.feed.key != Some(followed) {
            return;
        }
        match model::parse_frame(line) {
            Ok(Frame::Message(message)) => self.push_message(followed.0, message, cx),
            Ok(Frame::Dropped(_)) => {
                self.feed.stream = None;
                self.sync_feed(cx);
            }
            Err(_) => {}
        }
    }

    fn push_message(&mut self, run: u64, message: Message, cx: &mut Context<Self>) {
        let newer = self.feed.messages.last().is_none_or(|m| message.seq > m.seq);
        if message.run != run || !newer {
            return;
        }
        self.feed.messages.push(message);
        let extra = self.feed.messages.len().saturating_sub(KEPT);
        self.feed.messages.drain(..extra);
        cx.notify();
    }

    fn on_feed_end(&mut self, followed: (u64, u64), err: &str, cx: &mut Context<Self>) {
        if self.feed.key != Some(followed) {
            return;
        }
        self.feed.ended = Some(ended_because(err, "the run ended"));
        self.feed.stream = None;
        cx.notify();
    }

    fn read_files(&mut self, cx: &mut Context<Self>) {
        let Some(workspace) = self.selected_run().map(|r| r.workspace) else { return };
        store::run(
            cx,
            args(&["flow", "files", "--workspace", &workspace.to_string(), "--json"]),
            move |this, result, cx| this.files_read(workspace, result, cx),
        );
    }

    fn files_read(&mut self, workspace: u64, result: anyhow::Result<String>, cx: &mut Context<Self>) {
        if self.selected_run().map(|r| r.workspace) != Some(workspace) {
            return;
        }
        let files = result.and_then(|out| model::parse_files(&out)).map_err(|e| e.to_string());
        if self.files.as_ref() != Some(&files) {
            self.files = Some(files);
            cx.notify();
        }
    }

    fn selected_run(&self) -> Option<&Run> {
        match &self.selected {
            Some(Selected::Run(id)) => self.runs.iter().find(|r| r.id == *id),
            _ => None,
        }
    }

    fn selected_flow(&self) -> Option<&FlowInfo> {
        match &self.selected {
            Some(Selected::Flow(key)) => self.listing.as_ref()?.flows.iter().find(|f| f.key() == *key),
            Some(Selected::Hidden(name)) => self.listing.as_ref()?.hidden.iter().find(|f| f.name == *name),
            _ => None,
        }
    }

    fn selection_exists(&self) -> bool {
        match &self.selected {
            Some(Selected::Run(id)) => self.runs.iter().any(|r| r.id == *id),
            Some(Selected::Broken(path)) => {
                self.listing.as_ref().is_some_and(|l| l.broken.iter().any(|b| b.path == *path))
            }
            Some(_) => self.selected_flow().is_some(),
            None => false,
        }
    }

    fn ensure_selection(&mut self) {
        if self.selection_exists() {
            return;
        }
        self.selected = self.runs.first().map(|r| Selected::Run(r.id)).or_else(|| {
            let flows = &self.listing.as_ref()?.flows;
            let flow = flows.iter().find(|f| f.default).or(flows.first())?;
            Some(Selected::Flow(flow.key()))
        });
    }

    fn select(&mut self, selected: Selected, cx: &mut Context<Self>) {
        if self.selected.as_ref() != Some(&selected) {
            self.selected = Some(selected);
            self.files = None;
            self.prompt = None;
            self.sync_feed(cx);
            cx.notify();
        }
    }

    pub(crate) fn select_workspace_run(&mut self, workspace: u64, cx: &mut Context<Self>) {
        if self.select_run_in(workspace, cx) {
            return;
        }
        cx.spawn(async move |this, cx| wait_for_run(this, workspace, cx).await).detach();
    }

    fn select_run_in(&mut self, workspace: u64, cx: &mut Context<Self>) -> bool {
        let Some(id) = self.runs.iter().find(|r| r.workspace == workspace).map(|r| r.id) else { return false };
        self.select(Selected::Run(id), cx);
        true
    }

    fn detail(&self, cx: &Context<Self>) -> AnyElement {
        match &self.selected {
            Some(Selected::Run(_)) => self.run_detail(cx),
            Some(Selected::Flow(_)) => self.flow_detail(false, cx),
            Some(Selected::Hidden(_)) => self.flow_detail(true, cx),
            Some(Selected::Broken(path)) => self.broken_page(path, cx).into_any_element(),
            None => ui::note("Select a run or a flow.").into_any_element(),
        }
    }

    fn run_detail(&self, cx: &Context<Self>) -> AnyElement {
        let Some(run) = self.selected_run() else { return ui::note("This run is over.").into_any_element() };
        self.run_page(run, cx).into_any_element()
    }

    fn flow_detail(&self, hidden: bool, cx: &Context<Self>) -> AnyElement {
        let Some(flow) = self.selected_flow() else { return ui::note("This flow is gone.").into_any_element() };
        self.flow_page(flow, hidden, cx).into_any_element()
    }

    fn detail_pane(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        div()
            .id("flows-detail")
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .p(px(18.))
            .child(div().w_full().child(self.detail(cx)))
    }
}

pub(crate) fn deliver<T: 'static, R: 'static>(
    cx: &mut Context<T>,
    task: Task<R>,
    then: impl FnOnce(&mut T, R, &mut Context<T>) + 'static,
) {
    cx.spawn(async move |this, cx| {
        let result = task.await;
        let _ = this.update(cx, |this, cx| then(this, result, cx));
    })
    .detach();
}

fn ended_because(err: &str, fallback: &str) -> String {
    let err = err.trim();
    if err.is_empty() { fallback.to_string() } else { err.to_string() }
}

async fn poll_while_alive(this: WeakEntity<FlowsPanel>, cx: &mut AsyncApp) {
    loop {
        cx.background_executor().timer(POLL).await;
        if this.update(cx, FlowsPanel::poll).is_err() {
            break;
        }
    }
}

async fn wait_for_run(this: WeakEntity<FlowsPanel>, workspace: u64, cx: &mut AsyncApp) {
    for _ in 0..20 {
        cx.background_executor().timer(Duration::from_millis(500)).await;
        let found = this.update(cx, |this, cx| this.select_run_in(workspace, cx));
        if !matches!(found, Ok(false)) {
            break;
        }
    }
}

impl Render for FlowsPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().flex().child(self.sidebar(cx)).child(self.detail_pane(cx))
    }
}

pub(crate) fn build(window: &mut Window, cx: &mut gpui::App) -> AnyView {
    cx.new(|cx| FlowsPanel::new(window, cx)).into()
}
