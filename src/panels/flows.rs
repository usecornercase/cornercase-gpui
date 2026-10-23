//! Teams of agents run by an orchestrator (`cornercase flow`): the flows (built in, yours and the
//! project's) and their runs, live. A run shows its team (a click goes to a member's pane), its
//! plan lit up where the work stands, the messages the team sent each other as a feed, and the
//! files of its folder. A flow shows its plan and roles, and the actions `cornercase flow` offers:
//! start, set default, order, share, duplicate, edit its file, delete, hide and restore.
//!
//! Runs follow `flow runs --follow`; the selected run's messages follow `flow messages --follow
//! --json`; the list and the run's files are read again every few seconds while the sheet shows.

mod actions;
mod diagram;
mod import;
pub(crate) mod model;
mod start;

use std::collections::HashSet;
use std::time::Duration;

use gpui::{
    AnyElement, AnyView, AppContext as _, ClickEvent, Context, Div, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _, px,
};

use self::model::{Files, FlowInfo, Frame, Listing, Message, Run};
use crate::cli::Stream;
use crate::panels::PanelKind;
use crate::store::{self, Store, args};
use crate::theme::colours;
use crate::ui;

/// How often the list and the selected run's files are read again while the sheet shows.
const POLL: Duration = Duration::from_secs(3);
/// The messages kept for the selected run, as the server keeps them.
const KEPT: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Selected {
    Run(u64),
    /// A listed flow, by `FlowInfo::key`.
    Flow(String),
    /// A deleted built-in, by name.
    Hidden(String),
    /// A file that cannot be read, by path.
    Broken(String),
}

/// The selected run's messages, as `flow messages --follow` gives them.
#[derive(Default)]
struct Feed {
    /// The run and workspace followed.
    key: Option<(u64, u64)>,
    messages: Vec<Message>,
    /// Why the stream stopped (the run ended, an error).
    ended: Option<String>,
    _stream: Option<Stream>,
}

pub(crate) struct FlowsPanel {
    /// The project whose flows are listed too (the shown one when last read).
    project: Option<u64>,
    listing: Option<Listing>,
    list_error: Option<String>,
    listing_busy: bool,
    runs: Vec<Run>,
    runs_error: Option<String>,
    _runs_stream: Option<Stream>,
    selected: Option<Selected>,
    feed: Feed,
    files: Option<Result<Files, String>>,
    /// Messages shown whole, by `seq`.
    expanded: HashSet<u64>,
    /// The role whose prompt the flow page shows.
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
            _runs_stream: None,
            selected: None,
            feed: Feed::default(),
            files: None,
            expanded: HashSet::new(),
            prompt: None,
        };
        this.follow_runs(cx);
        this.refresh(cx);
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL).await;
                let alive = this.update(cx, |this, cx| {
                    if Store::get(cx).read(cx).sheet == Some(PanelKind::Flows) {
                        this.refresh(cx);
                        this.read_files(cx);
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();
        this
    }

    // -----------------------------------------------------------------------------------------
    // Reading.

    /// Reads the flows again, the shown project's included.
    pub(crate) fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.listing_busy {
            return;
        }
        self.listing_busy = true;
        let project = Store::get(cx).read(cx).session.as_ref().and_then(|s| s.shown().project);
        let mut a = args(&["flow", "list", "--json"]);
        if let Some(p) = project {
            a.extend(["--project".into(), p.to_string()]);
        }
        store::run(cx, a, move |this, result, cx| {
            this.listing_busy = false;
            this.project = project;
            match result.and_then(|out| model::parse_listing(&out)) {
                Ok(listing) => {
                    if this.listing.as_ref() != Some(&listing) {
                        this.listing = Some(listing);
                        this.ensure_selection();
                    }
                    this.list_error = None;
                }
                Err(e) => this.list_error = Some(e.to_string()),
            }
            cx.notify();
        });
    }

    fn follow_runs(&mut self, cx: &mut Context<Self>) {
        self._runs_stream = store::follow_until(
            cx,
            args(&["flow", "runs", "--follow"]),
            |this, line, cx| match model::parse_runs(&line) {
                Ok(runs) => {
                    this.runs = runs.runs;
                    this.runs_error = None;
                    this.ensure_selection();
                    this.sync_feed(cx);
                    cx.notify();
                }
                Err(e) => this.runs_error = Some(format!("cannot read `flow runs`: {e}")),
            },
            |this, err, cx| {
                let err = err.trim();
                this.runs_error =
                    Some(if err.is_empty() { "stopped following the runs".into() } else { err.to_string() });
                this._runs_stream = None;
                cx.notify();
                // The server restarted or went away: follow again shortly.
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(POLL).await;
                    let _ = this.update(cx, FlowsPanel::follow_runs);
                })
                .detach();
            },
        );
    }

    /// Follows the selected run's messages, and only its.
    fn sync_feed(&mut self, cx: &mut Context<Self>) {
        let key = self.selected_run().filter(|r| !r.closed()).map(|r| (r.run, r.workspace));
        if key == self.feed.key && (key.is_none() || self.feed._stream.is_some() || self.feed.ended.is_some()) {
            return;
        }
        let Some((run, workspace)) = key else {
            self.feed = Feed::default();
            return;
        };
        let keep = self.feed.key == key;
        let messages = if keep { std::mem::take(&mut self.feed.messages) } else { Vec::new() };
        if !keep {
            self.expanded.clear();
        }
        self.feed = Feed { key, messages, ended: None, _stream: None };
        self.feed._stream = store::follow_until(
            cx,
            args(&["flow", "messages", "--workspace", &workspace.to_string(), "--follow", "--json"]),
            move |this, line, cx| {
                if this.feed.key != Some((run, workspace)) {
                    return;
                }
                match model::parse_frame(&line) {
                    Ok(Frame::Message(message)) => {
                        let newer = this.feed.messages.last().is_none_or(|m| message.seq > m.seq);
                        if message.run == run && newer {
                            this.feed.messages.push(message);
                            let extra = this.feed.messages.len().saturating_sub(KEPT);
                            this.feed.messages.drain(..extra);
                            cx.notify();
                        }
                    }
                    // It fell behind: read again what the server keeps.
                    Ok(Frame::Dropped(_)) => {
                        this.feed._stream = None;
                        this.sync_feed(cx);
                    }
                    Err(_) => {}
                }
            },
            move |this, err, cx| {
                if this.feed.key == Some((run, workspace)) {
                    let err = err.trim();
                    this.feed.ended = Some(if err.is_empty() { "the run ended".into() } else { err.to_string() });
                    this.feed._stream = None;
                    cx.notify();
                }
            },
        );
        self.read_files(cx);
    }

    /// The selected run's folder (also after it ended).
    fn read_files(&mut self, cx: &mut Context<Self>) {
        let Some(workspace) = self.selected_run().map(|r| r.workspace) else { return };
        store::run(
            cx,
            args(&["flow", "files", "--workspace", &workspace.to_string(), "--json"]),
            move |this, r, cx| {
                if this.selected_run().map(|r| r.workspace) != Some(workspace) {
                    return;
                }
                let files = r.and_then(|out| model::parse_files(&out)).map_err(|e| e.to_string());
                if this.files.as_ref() != Some(&files) {
                    this.files = Some(files);
                    cx.notify();
                }
            },
        );
    }

    // -----------------------------------------------------------------------------------------
    // Selection.

    fn selected_run(&self) -> Option<&Run> {
        match &self.selected {
            Some(Selected::Run(id)) => self.runs.iter().find(|r| r.run == *id),
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

    /// Keeps a selection that still exists, else picks the first run, else the default flow.
    fn ensure_selection(&mut self) {
        let exists = match &self.selected {
            Some(Selected::Run(id)) => self.runs.iter().any(|r| r.run == *id),
            Some(Selected::Broken(path)) => {
                self.listing.as_ref().is_some_and(|l| l.broken.iter().any(|b| b.path == *path))
            }
            Some(_) => self.selected_flow().is_some(),
            None => false,
        };
        if exists {
            return;
        }
        self.selected = self.runs.first().map(|r| Selected::Run(r.run)).or_else(|| {
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

    /// Selects a run once `flow runs` lists it (right after starting one).
    pub(crate) fn select_workspace_run(&mut self, workspace: u64, cx: &mut Context<Self>) {
        if let Some(run) = self.runs.iter().find(|r| r.workspace == workspace) {
            let id = run.run;
            self.select(Selected::Run(id), cx);
            return;
        }
        cx.spawn(async move |this, cx| {
            for _ in 0..20 {
                cx.background_executor().timer(Duration::from_millis(500)).await;
                let found = this.update(cx, |this, cx| {
                    let id = this.runs.iter().find(|r| r.workspace == workspace).map(|r| r.run);
                    id.inspect(|id| this.select(Selected::Run(*id), cx)).is_some()
                });
                if !matches!(found, Ok(false)) {
                    break;
                }
            }
        })
        .detach();
    }

    // -----------------------------------------------------------------------------------------
    // Drawing: the list on the left.

    fn render_list(&self, cx: &mut Context<Self>) -> Div {
        let store = Store::get(cx);
        let session = store.read(cx).session.clone();
        let mut list = div().flex().flex_col().gap(px(1.));

        list = list.child(ui::section("Runs"));
        if self.runs.is_empty() {
            list = list.child(ui::dim("No flow is running.").px(px(8.)).pb(px(4.)));
        }
        if let Some(err) = &self.runs_error {
            list = list.child(div().px(px(8.)).text_size(px(11.)).text_color(colours::failed()).child(err.clone()));
        }
        for run in &self.runs {
            let id = run.run;
            let selected = self.selected == Some(Selected::Run(id));
            let place = session.as_ref().and_then(|s| s.workspace(run.workspace)).map_or_else(
                || format!("workspace {}", run.workspace),
                |(p, w)| format!("{} › {}", p.name, w.branch.clone().unwrap_or_else(|| w.name.clone())),
            );
            let state = if run.closed() {
                "closed".to_string()
            } else if run.ended {
                "ended".to_string()
            } else {
                run.summary()
            };
            list = list.child(
                ui::row(SharedString::from(format!("run-{id}")), selected)
                    .items_start()
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.select(Selected::Run(id), cx)))
                    .child(ui::badge(if run.closed() { None } else { run.loudest() }).pt(px(1.)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .flex()
                                    .justify_between()
                                    .gap(px(6.))
                                    .child(div().truncate().text_size(px(13.)).child(run.flow.name.clone()))
                                    .child(ui::dim(state).flex_none()),
                            )
                            .child(ui::dim(place).truncate())
                            .child(ui::dim(run.headline().to_string()).truncate()),
                    ),
            );
        }

        list = list.child(ui::section("Flows"));
        match (&self.listing, &self.list_error) {
            (_, Some(err)) => {
                list = list.child(div().px(px(8.)).text_size(px(11.)).text_color(colours::failed()).child(err.clone()));
            }
            (None, None) => list = list.child(ui::dim("Reading the flows…").px(px(8.))),
            _ => {}
        }
        let Some(listing) = &self.listing else { return list };
        for (i, flow) in listing.flows.iter().enumerate() {
            let key = flow.key();
            let selected = self.selected.as_ref() == Some(&Selected::Flow(key.clone()));
            let second = match flow.problems.first() {
                Some(problem) => {
                    div().text_size(px(11.)).text_color(colours::failed()).truncate().child(problem.clone())
                }
                None => ui::dim(flow.description.clone()).truncate(),
            };
            list = list.child(
                ui::row(("flow", i), selected)
                    .on_click(
                        cx.listener(move |this, _: &ClickEvent, _, cx| this.select(Selected::Flow(key.clone()), cx)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .child(div().truncate().text_size(px(13.)).child(flow.name.clone()))
                                    .children(flow.tags().into_iter().map(tag))
                                    .child(div().flex_1())
                                    .child(ui::dim(format!("{} agents", flow.agents())).flex_none()),
                            )
                            .child(second),
                    ),
            );
        }
        if !listing.hidden.is_empty() {
            list = list.child(ui::section("Deleted built-ins"));
            for (i, flow) in listing.hidden.iter().enumerate() {
                let name = flow.name.clone();
                let selected = self.selected.as_ref() == Some(&Selected::Hidden(name.clone()));
                list = list.child(
                    ui::row(("hidden", i), selected)
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.select(Selected::Hidden(name.clone()), cx);
                        }))
                        .child(div().flex_1().text_size(px(13.)).text_color(colours::muted()).child(flow.name.clone())),
                );
            }
        }
        if !listing.broken.is_empty() {
            list = list.child(ui::section("Cannot be read"));
            for (i, broken) in listing.broken.iter().enumerate() {
                let path = broken.path.clone();
                let selected = self.selected.as_ref() == Some(&Selected::Broken(path.clone()));
                list = list.child(
                    ui::row(("broken", i), selected)
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.select(Selected::Broken(path.clone()), cx);
                        }))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(div().truncate().text_size(px(13.)).child(file_name(&broken.path)))
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(colours::failed())
                                        .truncate()
                                        .child(broken.error.clone()),
                                ),
                        ),
                );
            }
        }
        list
    }

    // -----------------------------------------------------------------------------------------
    // Drawing: a run.

    fn render_run(&self, run: &Run, cx: &mut Context<Self>) -> Div {
        let store = Store::get(cx);
        let session = store.read(cx).session.clone();
        let place = session.as_ref().and_then(|s| s.workspace(run.workspace)).map(|(p, w)| {
            let branch = w.branch.as_ref().filter(|b| **b != w.name).map(|b| format!(" · {b}")).unwrap_or_default();
            format!("{} › {}{branch}", p.name, w.name)
        });
        let state = if run.closed() {
            ("agents closed", colours::faint())
        } else if run.ended {
            ("reached its end", colours::accent())
        } else {
            ("running", colours::working())
        };
        let mut page = div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(ui::title(run.flow.name.clone()).text_size(px(16.)))
                    .child(div().text_size(px(12.)).text_color(state.1).child(state.0))
                    .child(div().flex_1())
                    .child(self.run_buttons(run, cx)),
            )
            .child(ui::dim(format!(
                "{} · {} · {} message{}",
                place.unwrap_or_else(|| format!("workspace {}", run.workspace)),
                run.summary(),
                run.messages,
                if run.messages == 1 { "" } else { "s" }
            )))
            .child(
                div()
                    .mt(px(8.))
                    .p(px(10.))
                    .rounded(px(6.))
                    .bg(colours::terminal())
                    .border_1()
                    .border_color(colours::border())
                    .text_size(px(12.))
                    .line_clamp(8)
                    .child(run.task.clone()),
            );

        page = page.child(ui::section("Team"));
        page = page.child(self.render_team(run));
        page = page.child(ui::section("Plan"));
        page = page.child(diagram::diagram(&run.flow, Some(run)));
        page = page.child(ui::section(format!("Messages ({})", self.feed.messages.len())));
        page = page.child(self.render_feed(run, cx));
        page = page.child(ui::section("Files"));
        page.child(self.render_files(run, cx))
    }

    fn run_buttons(&self, run: &Run, cx: &mut Context<Self>) -> Div {
        let workspace = run.workspace;
        let mut buttons = div().flex().gap(px(6.));
        if run.closed() {
            return buttons
                .child(ui::primary_button("run-reopen", "Reopen agents").on_click(cx.listener(
                    move |this, _: &ClickEvent, _, cx| {
                        this.run_cmd(args(&["agents", "reopen", "--workspace", &workspace.to_string()]), "Reopening the team", cx);
                    },
                )))
                .child(ui::danger_button("run-forget", "Forget").on_click(cx.listener(
                    move |this, _: &ClickEvent, window, cx| {
                        this.ask(
                            "Forget this run?",
                            "Its closed agents are dropped for good and the run ends. The worktree and its branch stay.",
                            "Forget",
                            args(&["agents", "forget", "--workspace", &workspace.to_string(), "--yes"]),
                            window,
                            cx,
                        );
                    },
                )));
        }
        // The orchestrator, else a member still open: closing either closes the whole team.
        let alive = |m: &&model::Member| !m.closed && m.pane.is_some();
        let lead = run.orchestrator().filter(alive).or_else(|| run.roles.iter().find(alive)).and_then(|m| m.pane);
        if let Some(pane) = lead {
            buttons = buttons
                .child(ui::button("run-show", "Show orchestrator").on_click(move |_: &ClickEvent, _, cx| {
                    Store::get(cx).update(cx, |s, cx| {
                        s.focus("pane", pane);
                        s.close_sheet(cx);
                    });
                }))
                .child(ui::danger_button("run-stop", "Stop").on_click(cx.listener(
                    move |this, _: &ClickEvent, window, cx| {
                        this.ask(
                            "Stop this run?",
                            "The orchestrator and its whole team close, and the run ends. Nothing is removed from disk: the worktree and its branch stay.",
                            "Stop",
                            args(&["close", "--agent", "--pane", &pane.to_string()]),
                            window,
                            cx,
                        );
                    },
                )));
        }
        buttons
    }

    fn render_team(&self, run: &Run) -> Div {
        let mut team = div().flex().flex_col().gap(px(1.));
        for (i, member) in run.roles.iter().enumerate() {
            let spec = run.flow.role(&member.role);
            let agent = spec
                .map(|r| {
                    [Some(r.agent.clone()), r.model.clone(), r.effort.clone()].into_iter().flatten().collect::<Vec<_>>()
                })
                .unwrap_or_default()
                .join(" · ");
            let pane = member.pane.filter(|_| !member.closed);
            let status = if member.closed {
                "closed".to_string()
            } else {
                member.status.clone().unwrap_or_else(|| "—".into())
            };
            let colour = diagram::role_colour(&run.flow, &member.role);
            let mut row = ui::row(("member", i), false)
                .when(member.closed, |d| d.opacity(0.55))
                .child(ui::badge(if member.closed { None } else { member.status.as_deref() }))
                .child(
                    div()
                        .w(px(150.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(div().size(px(7.)).rounded_full().bg(colour))
                        .child(div().truncate().text_color(colour).child(member.role.clone())),
                )
                .child(
                    div()
                        .w(px(200.))
                        .flex_none()
                        .truncate()
                        .text_size(px(12.))
                        .text_color(colours::muted())
                        .child(agent),
                )
                .child(div().w(px(80.)).flex_none().text_size(px(12.)).child(status))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .gap(px(6.))
                        .children(member.awaited.as_ref().map(|a| tag_owned(format!("work {a}"))))
                        .children(member.branch.as_ref().map(|b| ui::dim(format!("⎇ {b}")).truncate())),
                )
                .child(ui::dim(pane.map(|p| format!("pane {p}")).unwrap_or_default()).flex_none());
            if let Some(pane) = pane {
                row = row.on_click(move |_: &ClickEvent, _, cx| {
                    Store::get(cx).update(cx, |s, cx| {
                        s.focus("pane", pane);
                        s.close_sheet(cx);
                    });
                });
            } else {
                row = row.cursor_default();
            }
            team = team.child(row);
        }
        team
    }

    fn render_feed(&self, run: &Run, cx: &mut Context<Self>) -> Div {
        let mut feed = div().flex().flex_col().gap(px(4.));
        if run.closed() {
            return feed
                .child(ui::dim("Messages are kept while the run is live; reopen the agents to follow it again."));
        }
        if self.feed.messages.is_empty() {
            feed = feed.child(ui::dim(
                "Nothing sent yet. The orchestrator's sends, keys, reads and waits appear here as they happen.",
            ));
        }
        let now = model::now();
        for message in self.feed.messages.iter().rev() {
            let seq = message.seq;
            let whole = self.expanded.contains(&seq);
            let from = diagram::role_colour(&run.flow, &message.from);
            let to = diagram::role_colour(&run.flow, &message.to);
            let edge = match message.edge {
                Some(model::Edge::Forward { stage }) => Some(format!("stage {} → {}", stage + 1, stage + 2)),
                Some(model::Edge::Loop { stage }) => {
                    run.flow.stages.get(stage).and_then(|s| s.back.as_ref()).map(|b| format!("↺ {}", b.when))
                }
                Some(model::Edge::Outside { .. }) => Some("outside the plan".into()),
                None => None,
            };
            let text = message.text.clone().unwrap_or_default();
            let shown: String = if whole { text.clone() } else { text.chars().take(1200).collect() };
            let cut = message.bytes as usize > text.len();
            feed = feed.child(
                div()
                    .id(("message", seq))
                    .p(px(8.))
                    .rounded(px(6.))
                    .bg(colours::terminal())
                    .border_1()
                    .border_color(colours::border())
                    .cursor_pointer()
                    .hover(|s| s.border_color(colours::muted()))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if !this.expanded.remove(&seq) {
                            this.expanded.insert(seq);
                        }
                        cx.notify();
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .text_size(px(12.))
                            .child(div().text_color(from).font_weight(FontWeight::SEMIBOLD).child(message.from.clone()))
                            .child(div().text_color(colours::faint()).child("→"))
                            .child(div().text_color(to).font_weight(FontWeight::SEMIBOLD).child(message.to.clone()))
                            .child(div().text_color(colours::muted()).child(message.verb().to_string()))
                            .children(edge.map(tag_owned))
                            .child(div().flex_1())
                            .when(!text.is_empty(), |d| {
                                d.child(ui::icon_button(("copy-message", seq), "⧉").text_size(px(12.)).on_click(
                                    move |_: &ClickEvent, _, cx| {
                                        cx.stop_propagation();
                                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.clone()));
                                    },
                                ))
                            })
                            .child(ui::dim(format!("#{seq} · {}", model::ago(&message.time, now)))),
                    )
                    .when(!shown.is_empty(), |d| {
                        d.child(
                            div()
                                .pt(px(4.))
                                .text_size(px(12.))
                                .text_color(colours::text())
                                .when(!whole, |d| d.line_clamp(4))
                                .child(shown),
                        )
                    })
                    .when(cut && whole, |d| {
                        d.child(ui::dim(format!("cut at 32 KiB of {}", model::size(message.bytes))))
                    }),
            );
        }
        if let Some(ended) = &self.feed.ended {
            feed = feed.child(ui::dim(format!("Not following: {ended}")));
        }
        feed
    }

    fn render_files(&self, run: &Run, _cx: &mut Context<Self>) -> Div {
        let mut list = div().flex().flex_col().gap(px(1.));
        if let Some(dir) = &run.dir {
            let dir = dir.clone();
            list = list.child(
                div().flex().items_center().gap(px(8.)).pb(px(4.)).child(ui::dim(dir.clone()).truncate()).child(
                    ui::button("run-folder", "Open folder").on_click(move |_: &ClickEvent, _, cx| {
                        cx.open_with_system(std::path::Path::new(&dir));
                    }),
                ),
            );
        }
        match &self.files {
            None => list.child(ui::dim("Reading the folder…")),
            Some(Err(e)) => list.child(div().text_size(px(11.)).text_color(colours::failed()).child(e.clone())),
            Some(Ok(files)) if files.files.is_empty() => {
                list.child(ui::dim("No files yet: the roles write their answers here."))
            }
            Some(Ok(files)) => {
                let now = model::now();
                for (i, file) in files.files.iter().enumerate() {
                    let path = file.path.clone();
                    list = list.child(
                        ui::row(("run-file", i), false)
                            .on_click(move |_: &ClickEvent, _, cx| cx.open_with_system(std::path::Path::new(&path)))
                            .child(div().flex_1().truncate().text_size(px(12.)).child(file.name.clone()))
                            .child(ui::dim(model::size(file.bytes)).w(px(70.)).flex_none())
                            .child(
                                ui::dim(file.modified.as_deref().map(|m| model::ago(m, now)).unwrap_or_default())
                                    .w(px(40.))
                                    .flex_none(),
                            ),
                    );
                }
                if files.more {
                    list = list.child(ui::dim("Only the newest 200 files are listed."));
                }
                list
            }
        }
    }

    // -----------------------------------------------------------------------------------------
    // Drawing: a flow.

    fn render_flow(&self, flow: &FlowInfo, hidden: bool, cx: &mut Context<Self>) -> Div {
        let mut page = div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(ui::title(flow.name.clone()).text_size(px(16.)))
                    .children(flow.tags().into_iter().map(tag))
                    .when(hidden, |d| d.child(tag("deleted")))
                    .child(div().flex_1())
                    .child(ui::dim(format!("{} agents", flow.agents()))),
            )
            .when(!flow.description.is_empty(), |d| {
                d.child(div().text_size(px(13.)).text_color(colours::muted()).child(flow.description.clone()))
            })
            .children(flow.path.as_ref().map(|p| ui::dim(p.clone())))
            .children(
                flow.problems.iter().map(|p| {
                    div().text_size(px(12.)).text_color(colours::failed()).child(format!("Cannot start: {p}"))
                }),
            )
            .children(
                flow.check.as_ref().map(|c| div().text_size(px(12.)).text_color(colours::failed()).child(c.clone())),
            )
            .child(self.flow_buttons(flow, hidden, cx));

        page = page.child(ui::section("Plan")).child(diagram::diagram(&flow.flow, None));
        page = page.child(ui::section("Roles"));
        let spec = &flow.flow;
        for (i, role) in std::iter::once(&spec.orchestrator).chain(&spec.roles).enumerate() {
            let name = role.name.clone();
            let open = self.prompt.as_deref() == Some(role.name.as_str());
            let colour = diagram::role_colour(spec, &role.name);
            let mut facts: Vec<String> =
                [Some(role.agent.clone()), role.model.clone(), role.effort.clone()].into_iter().flatten().collect();
            facts.push(if role.edits { "edits".into() } else { "read only".into() });
            if role.worktree == "own" {
                facts.push("own worktree".into());
            }
            if !role.env.is_empty() {
                facts.push(format!("env {}", role.env.keys().cloned().collect::<Vec<_>>().join(", ")));
            }
            if !role.args.is_empty() {
                facts.push(format!("args {}", role.args.join(" ")));
            }
            page = page.child(
                ui::row(("role", i), open)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.prompt =
                            if this.prompt.as_deref() == Some(name.as_str()) { None } else { Some(name.clone()) };
                        cx.notify();
                    }))
                    .child(div().text_color(colours::faint()).text_size(px(10.)).child(if open {
                        "▾"
                    } else {
                        "▸"
                    }))
                    .child(
                        div()
                            .w(px(150.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(div().size(px(7.)).rounded_full().bg(colour))
                            .child(div().truncate().text_color(colour).child(role.name.clone())),
                    )
                    .child(
                        div()
                            .flex_1()
                            .truncate()
                            .text_size(px(12.))
                            .text_color(colours::muted())
                            .child(facts.join(" · ")),
                    ),
            );
            if open {
                page = page.child(
                    div()
                        .ml(px(24.))
                        .mb(px(6.))
                        .p(px(10.))
                        .rounded(px(6.))
                        .bg(colours::terminal())
                        .border_1()
                        .border_color(colours::border())
                        .text_size(px(12.))
                        .child(if role.prompt.is_empty() {
                            "(no prompt of its own)".to_string()
                        } else {
                            role.prompt.clone()
                        }),
                );
            }
        }
        if !spec.notes.is_empty() {
            page = page.child(ui::section("Notes")).child(div().text_size(px(12.)).child(spec.notes.clone()));
        }
        page
    }

    fn render_broken(&self, path: &str, cx: &mut Context<Self>) -> Div {
        let error = self
            .listing
            .as_ref()
            .and_then(|l| l.broken.iter().find(|b| b.path == path))
            .map(|b| b.error.clone())
            .unwrap_or_default();
        let (open, delete) = (path.to_string(), path.to_string());
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(ui::title(file_name(path)).text_size(px(16.)))
            .child(ui::dim(path.to_string()))
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(colours::failed())
                    .child(format!("It cannot be read as a flow, so it has no diagram: {error}")),
            )
            .child(
                div()
                    .flex()
                    .gap(px(6.))
                    .pt(px(6.))
                    .child(
                        ui::button("broken-open", "Open file")
                            .on_click(move |_: &ClickEvent, _, cx| cx.open_with_system(std::path::Path::new(&open))),
                    )
                    .child(ui::danger_button("broken-delete", "Delete file").on_click(cx.listener(
                        move |this, _: &ClickEvent, window, cx| {
                            this.ask(
                                "Delete this file?",
                                format!("{delete} is deleted from disk."),
                                "Delete",
                                args(&["flow", "delete", "--path", &delete]),
                                window,
                                cx,
                            );
                        },
                    ))),
            )
    }
}

/// A small muted tag beside a name.
fn tag(text: &'static str) -> Div {
    tag_owned(text.to_string())
}

fn tag_owned(text: String) -> Div {
    div()
        .flex_none()
        .px(px(5.))
        .rounded(px(4.))
        .bg(colours::selected())
        .text_size(px(10.))
        .text_color(colours::muted())
        .child(text)
}

fn file_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

impl Render for FlowsPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let detail: AnyElement = match self.selected.clone() {
            Some(Selected::Run(_)) => match self.selected_run().cloned() {
                Some(run) => self.render_run(&run, cx).into_any_element(),
                None => ui::note("This run is over.").into_any_element(),
            },
            Some(Selected::Flow(_)) => match self.selected_flow().cloned() {
                Some(flow) => self.render_flow(&flow, false, cx).into_any_element(),
                None => ui::note("This flow is gone.").into_any_element(),
            },
            Some(Selected::Hidden(_)) => match self.selected_flow().cloned() {
                Some(flow) => self.render_flow(&flow, true, cx).into_any_element(),
                None => ui::note("This flow is gone.").into_any_element(),
            },
            Some(Selected::Broken(path)) => self.render_broken(&path, cx).into_any_element(),
            None => ui::note("Select a run or a flow.").into_any_element(),
        };
        div()
            .size_full()
            .flex()
            .child(
                div()
                    .w(px(330.))
                    .h_full()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .border_r_1()
                    .border_color(colours::border())
                    .child(
                        div()
                            .flex()
                            .gap(px(6.))
                            .p(px(10.))
                            .border_b_1()
                            .border_color(colours::border())
                            .child(ui::primary_button("flows-start", "Start a flow…").on_click(cx.listener(
                                |this, _: &ClickEvent, window, cx| {
                                    let name = this.selected_flow().map(FlowInfo::key);
                                    this.open_start(name, window, cx);
                                },
                            )))
                            .child(ui::button("flows-import", "Import…").on_click(
                                cx.listener(|this, _: &ClickEvent, window, cx| this.open_import(window, cx)),
                            )),
                    )
                    .child(
                        div()
                            .id("flows-list")
                            .flex_1()
                            .overflow_y_scroll()
                            .px(px(8.))
                            .pb(px(12.))
                            .child(self.render_list(cx)),
                    ),
            )
            .child(
                div()
                    .id("flows-detail")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_y_scroll()
                    .p(px(18.))
                    .child(div().w_full().child(detail)),
            )
    }
}

pub(crate) fn build(window: &mut Window, cx: &mut gpui::App) -> AnyView {
    cx.new(|cx| FlowsPanel::new(window, cx)).into()
}
