//! The search sheet, as the TUI's: find a group, project, workspace or tab by name as you type,
//! or switch to "in sessions" and find where some text was said (`cornercase search --json`):
//! agents' transcripts and every pane's history, grouped by pane. `Tab` switches, the arrows move,
//! `Enter` or a click goes there. The query is not kept once the sheet closes.

use std::ops::Range;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gpui::{
    AnyElement, AnyView, AppContext as _, ClickEvent, Context, Entity, Focusable as _, FontWeight, HighlightStyle,
    InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render, Rgba, ScrollHandle, SharedString,
    StatefulInteractiveElement as _, Styled as _, StyledText, Window, div, prelude::FluentBuilder as _, px,
};
use serde::Deserialize;

use crate::model::Session;
use crate::panels::PanelKind;
use crate::store::{self, Store, args};
use crate::theme::colours;
use crate::ui::{self, TextField, TextFieldEvent};

/// How many names the list shows at most.
const MAX_NAMES: usize = 200;
/// How long typing must pause before a sessions search runs: each one reads every pane.
const DEBOUNCE: Duration = Duration::from_millis(220);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Mode {
    #[default]
    Names,
    Sessions,
}

pub(crate) struct SearchPanel {
    field: Entity<TextField>,
    mode: Mode,
    /// The highlighted result: a name, or a match in sessions.
    selected: usize,
    scroll: ScrollHandle,
    /// Whether the sheet is showing, to start afresh each time it opens.
    open: bool,
    sessions: Sessions,
}

/// The state of the "in sessions" search: the query asked last and its answer.
#[derive(Default)]
struct Sessions {
    generation: u64,
    /// The query whose answer is awaited or held.
    query: String,
    searching: bool,
    found: Option<Found>,
    error: Option<String>,
    /// `Enter` came before the answer: go to the first match when it does.
    enter: bool,
}

impl SearchPanel {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let field = cx.new(|cx| TextField::new("Search…", cx));
        cx.subscribe_in(&field, window, |this, _, event, window, cx| match event {
            TextFieldEvent::Changed => this.changed(cx),
            TextFieldEvent::Submit => this.submit(cx),
            TextFieldEvent::Cancel => this.close(window, cx),
        })
        .detach();
        let store = Store::get(cx);
        cx.observe_in(&store, window, |this, store, window, cx| {
            let open = store.read(cx).sheet == Some(PanelKind::Search);
            if open && !this.open {
                this.reset(window, cx);
            }
            this.open = open;
        })
        .detach();
        window.focus(&field.read(cx).focus_handle(cx));
        Self {
            field,
            mode: Mode::Names,
            selected: 0,
            scroll: ScrollHandle::new(),
            open: true,
            sessions: Sessions::default(),
        }
    }

    /// A fresh search, as each opening of the sheet starts.
    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.field.update(cx, TextField::clear);
        self.set_mode(Mode::Names, cx);
        window.focus(&self.field.read(cx).focus_handle(cx));
    }

    fn close(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        Store::get(cx).update(cx, Store::close_sheet);
    }

    fn query(&self, cx: &Context<Self>) -> String {
        self.field.read(cx).text().trim().to_string()
    }

    fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        self.mode = mode;
        self.selected = 0;
        self.sessions = Sessions { generation: self.sessions.generation + 1, ..Sessions::default() };
        if mode == Mode::Sessions {
            self.changed(cx);
        }
        cx.notify();
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        self.selected = 0;
        self.scroll.scroll_to_item(0);
        if self.mode == Mode::Sessions {
            self.schedule(cx);
        }
        cx.notify();
    }

    /// Asks `cornercase search` once typing pauses; a newer query makes an older answer moot.
    fn schedule(&mut self, cx: &mut Context<Self>) {
        let query = self.query(cx);
        self.sessions.generation += 1;
        self.sessions.enter = false;
        if query.is_empty() {
            self.sessions = Sessions { generation: self.sessions.generation, ..Sessions::default() };
            return;
        }
        if query == self.sessions.query && (self.sessions.searching || self.sessions.found.is_some()) {
            return;
        }
        let generation = self.sessions.generation;
        self.sessions.searching = true;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(DEBOUNCE).await;
            let _ = this.update(cx, |this, cx| {
                if this.sessions.generation == generation {
                    this.search(query, generation, cx);
                }
            });
        })
        .detach();
    }

    fn search(&mut self, query: String, generation: u64, cx: &mut Context<Self>) {
        self.sessions.query = query.clone();
        store::run_json(cx, args(&["search", "--json", "--", &query]), move |this, result, cx| {
            if this.sessions.generation != generation {
                return;
            }
            this.sessions.searching = false;
            match result.and_then(|v| serde_json::from_value::<Found>(v).map_err(anyhow::Error::from)) {
                Ok(found) => {
                    this.sessions.error = None;
                    this.sessions.found = Some(found);
                    if std::mem::take(&mut this.sessions.enter) {
                        this.submit(cx);
                    }
                }
                Err(e) => {
                    this.sessions.found = None;
                    this.sessions.error = Some(e.to_string());
                }
            }
            cx.notify();
        });
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        match self.mode {
            Mode::Names => {
                let names = self.names(cx);
                if let Some(hit) = names.get(self.selected.min(names.len().saturating_sub(1))) {
                    go(hit.candidate.goto, cx);
                }
            }
            Mode::Sessions => {
                let current = self.sessions.query == self.query(cx) && !self.sessions.searching;
                match self.sessions.found.as_ref().filter(|_| current) {
                    Some(found) => {
                        if let Some(hit) = found.matches.get(self.selected) {
                            go_to_pane(hit.pane, cx);
                        }
                    }
                    None => self.sessions.enter = !self.query(cx).is_empty(),
                }
            }
        }
    }

    fn names(&self, cx: &Context<Self>) -> Vec<Ranked> {
        let store = Store::get(cx);
        let Some(session) = &store.read(cx).session else { return Vec::new() };
        let mut ranked = rank(candidates(session), &self.query(cx));
        ranked.truncate(MAX_NAMES);
        ranked
    }

    fn count(&self, cx: &Context<Self>) -> usize {
        match self.mode {
            Mode::Names => self.names(cx).len(),
            Mode::Sessions => self.sessions.found.as_ref().map_or(0, |f| f.matches.len()),
        }
    }

    fn key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let count = self.count(cx);
        match event.keystroke.key.as_str() {
            "down" if count > 0 => self.selected = (self.selected + 1).min(count - 1),
            "up" => self.selected = self.selected.saturating_sub(1),
            "tab" => {
                let mode = if self.mode == Mode::Names { Mode::Sessions } else { Mode::Names };
                self.set_mode(mode, cx);
                cx.stop_propagation();
                return;
            }
            _ => return,
        }
        cx.stop_propagation();
        let row = match self.mode {
            Mode::Names => self.selected,
            Mode::Sessions => {
                self.sessions.found.as_ref().map_or(0, |f| row_of_match(&by_pane(&f.matches), self.selected))
            }
        };
        self.scroll.scroll_to_item(row);
        cx.notify();
    }

    // -----------------------------------------------------------------------------------------
    // Drawing.

    fn render_names(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let names = self.names(cx);
        self.selected = self.selected.min(names.len().saturating_sub(1));
        names
            .into_iter()
            .enumerate()
            .map(|(i, hit)| {
                let c = hit.candidate;
                let goto = c.goto;
                ui::row(SharedString::from(format!("name-{i}")), i == self.selected)
                    .child(kind_chip(c.kind))
                    .child(
                        div()
                            .flex_none()
                            .max_w(gpui::relative(0.55))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_color(colours::text())
                            .child(highlighted(c.name, hit.highlight)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(px(12.))
                            .text_color(colours::faint())
                            .child(c.context),
                    )
                    .child(ui::badge(c.status.as_deref()))
                    .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| go(goto, cx)))
                    .into_any_element()
            })
            .collect()
    }

    fn render_sessions(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(found) = &self.sessions.found else { return Vec::new() };
        self.selected = self.selected.min(found.matches.len().saturating_sub(1));
        let store = Store::get(cx);
        let session = store.read(cx).session.as_ref();
        let query = self.sessions.query.clone();
        let mut rows = Vec::new();
        let mut index = 0;
        for group in by_pane(&found.matches) {
            let pane = group.pane;
            let status = session.and_then(|s| s.pane(pane)).and_then(|p| p.status.clone());
            rows.push(
                div()
                    .id(SharedString::from(format!("pane-{pane}")))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .mt(px(if index == 0 { 2. } else { 10. }))
                    .px(px(8.))
                    .py(px(4.))
                    .rounded(px(5.))
                    .cursor_pointer()
                    .hover(|s| s.bg(colours::hover()))
                    .child(ui::badge(status.as_deref()))
                    .child(
                        div()
                            .flex_1()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(group.place.to_string()),
                    )
                    .child(chip(group.who.to_string(), colours::muted()))
                    .child(ui::dim(format!(
                        "pane {pane} · {} match{}",
                        group.hits.len(),
                        if group.hits.len() == 1 { "" } else { "es" }
                    )))
                    .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| go_to_pane(pane, cx)))
                    .into_any_element(),
            );
            for hit in group.hits {
                let i = index;
                index += 1;
                let when = match (&hit.written, hit.line) {
                    (Some(written), _) => parse_time(written).map(ago).unwrap_or_default(),
                    (None, Some(line)) => format!("line {line}"),
                    (None, None) => String::new(),
                };
                let range = find(&hit.text, &query);
                rows.push(
                    ui::row(SharedString::from(format!("match-{i}")), i == self.selected)
                        .ml(px(22.))
                        .child(source_chip(&hit.source))
                        .child(
                            div()
                                .flex_1()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_size(px(12.))
                                .text_color(colours::muted())
                                .child(highlighted(hit.text.clone(), range.into_iter().collect())),
                        )
                        .child(ui::dim(when))
                        .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| go_to_pane(pane, cx)))
                        .into_any_element(),
                );
            }
        }
        rows
    }

    fn render_status(&self, cx: &Context<Self>) -> (String, Option<String>) {
        let query = self.query(cx);
        match self.mode {
            Mode::Names if query.is_empty() => {
                ("Type to find a group, project, workspace or tab by its name or branch".into(), None)
            }
            Mode::Names => match self.names(cx).len() {
                0 => (format!("Nothing is called “{query}”"), None),
                n => (format!("{n} result{}", if n == 1 { "" } else { "s" }), None),
            },
            Mode::Sessions if query.is_empty() => {
                ("Type to find what agents were told and answered, and what every pane printed".into(), None)
            }
            Mode::Sessions => {
                if let Some(error) = &self.sessions.error {
                    return (error.clone(), None);
                }
                let note =
                    "Opens the pane; scrolling to the match needs usecornercase/cornercase-terminal#467".to_string();
                match &self.sessions.found {
                    _ if self.sessions.searching => ("Searching…".into(), None),
                    Some(found) if found.matches.is_empty() => (format!("Nobody said “{query}”"), None),
                    Some(found) => {
                        let panes = by_pane(&found.matches).len();
                        let n = found.matches.len();
                        let capped = if found.capped { " (more were left out)" } else { "" };
                        (
                            format!(
                                "{n} match{} in {panes} pane{}{capped}",
                                if n == 1 { "" } else { "es" },
                                if panes == 1 { "" } else { "s" }
                            ),
                            Some(note),
                        )
                    }
                    None => ("Searching…".into(), None),
                }
            }
        }
    }
}

impl Render for SearchPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.field.read(cx).focus_handle(cx).is_focused(window);
        let rows = match self.mode {
            Mode::Names => self.render_names(cx),
            Mode::Sessions => self.render_sessions(cx),
        };
        let (status, note) = self.render_status(cx);
        let mode = self.mode;
        let segment = |id: &'static str, label: &'static str, which: Mode, cx: &mut Context<Self>| {
            ui::segment(id, label, mode == which)
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    if this.mode != which {
                        this.set_mode(which, cx);
                    }
                    window.focus(&this.field.read(cx).focus_handle(cx));
                }))
                .into_any_element()
        };
        let segments = ui::segments(vec![
            segment("mode-names", "Names", Mode::Names, cx),
            segment("mode-sessions", "In sessions", Mode::Sessions, cx),
        ]);
        let searching = mode == Mode::Sessions && self.sessions.searching;
        div()
            .size_full()
            .flex()
            .flex_col()
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| this.key(e, cx)))
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(16.))
                    .pt(px(14.))
                    .child(
                        div().flex_1().child(ui::input_box(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(
                                    div()
                                        .text_color(if focused { colours::accent() } else { colours::faint() })
                                        .child(if mode == Mode::Names { "⌕" } else { "❯" }),
                                )
                                .child(div().flex_1().child(self.field.clone())),
                            focused,
                        )),
                    )
                    .child(segments),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.))
                    .px(px(18.))
                    .pt(px(8.))
                    .pb(px(6.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(if searching { colours::working() } else { colours::muted() })
                            .when(self.sessions.error.is_some() && mode == Mode::Sessions, |d| {
                                d.text_color(colours::failed())
                            })
                            .child(status),
                    )
                    .children(note.map(ui::dim)),
            )
            .child(
                div()
                    .id("search-results")
                    .flex_1()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .px(px(10.))
                    .pb(px(10.))
                    .opacity(if searching && !rows.is_empty() { 0.55 } else { 1. })
                    .children(rows),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .gap(px(16.))
                    .px(px(18.))
                    .py(px(8.))
                    .border_t_1()
                    .border_color(colours::border())
                    .children([("↑↓", "move"), ("↵", "go there"), ("⇥", "names or sessions"), ("esc", "close")].map(
                        |(key, what)| {
                            div()
                                .flex()
                                .gap(px(5.))
                                .text_size(px(11.))
                                .child(div().text_color(colours::muted()).child(key))
                                .child(div().text_color(colours::faint()).child(what))
                        },
                    )),
            )
    }
}

pub(crate) fn build(window: &mut Window, cx: &mut gpui::App) -> AnyView {
    cx.new(|cx| SearchPanel::new(window, cx)).into()
}

/// Shows a name's place in the window, as choosing it in the TUI does: a group is expanded and
/// its first project shown.
fn go(goto: Goto, cx: &mut gpui::App) {
    Store::get(cx).update(cx, |s, cx| {
        match goto {
            Goto::Group { id, first } => {
                s.cmd(args(&["expand", "--group", &id.to_string()]));
                if let Some(project) = first {
                    s.focus("project", project);
                }
            }
            Goto::Project(id) => s.focus("project", id),
            Goto::Workspace(id) => s.focus("workspace", id),
            Goto::Tab(id) => s.focus("tab", id),
        }
        s.close_sheet(cx);
    });
}

fn go_to_pane(pane: u64, cx: &mut gpui::App) {
    Store::get(cx).update(cx, |s, cx| {
        s.focus("pane", pane);
        s.close_sheet(cx);
    });
}

fn chip(text: impl Into<SharedString>, colour: Rgba) -> gpui::Div {
    div()
        .flex_none()
        .px(px(6.))
        .rounded(px(4.))
        .bg(colours::sidebar())
        .text_size(px(11.))
        .text_color(colour)
        .child(text.into())
}

fn kind_chip(kind: Kind) -> gpui::Div {
    let (label, colour) = match kind {
        Kind::Group => ("group", colours::waiting()),
        Kind::Project => ("project", colours::accent()),
        Kind::Workspace => ("workspace", colours::done()),
        Kind::Tab => ("tab", colours::muted()),
    };
    div().w(px(76.)).flex_none().child(chip(label, colour))
}

fn source_chip(source: &str) -> gpui::Div {
    let colour = match source {
        "prompt" => colours::accent(),
        "reply" => colours::done(),
        _ => colours::faint(),
    };
    div().w(px(56.)).flex_none().child(chip(source.to_string(), colour))
}

/// Text with the given byte ranges in the accent colour, bold.
fn highlighted(text: impl Into<SharedString>, ranges: Vec<Range<usize>>) -> StyledText {
    let style = HighlightStyle {
        color: Some(colours::accent().into()),
        font_weight: Some(FontWeight::BOLD),
        ..Default::default()
    };
    StyledText::new(text).with_highlights(ranges.into_iter().map(|r| (r, style)))
}

// ---------------------------------------------------------------------------------------------
// Names: what can be found, and how a query ranks it. Pure, so the tests need no window.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Group,
    Project,
    Workspace,
    Tab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Goto {
    /// A group, and the first project in it to show.
    Group {
        id: u64,
        first: Option<u64>,
    },
    Project(u64),
    Workspace(u64),
    Tab(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Candidate {
    kind: Kind,
    goto: Goto,
    /// What the row shows.
    name: String,
    /// Where it is: the group, project, or `project › workspace`.
    context: String,
    /// What a query is matched against: the name first, then branches and the workspace's.
    keys: Vec<String>,
    status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Ranked {
    candidate: Candidate,
    /// The byte ranges of the name that matched, to highlight.
    highlight: Vec<Range<usize>>,
}

/// Every group, project, workspace and tab of the session, in the sidebar's order: projects
/// gathered under their group where the group first shows.
fn candidates(session: &Session) -> Vec<Candidate> {
    let mut ordered: Vec<(Option<u64>, Vec<&crate::model::Project>)> = Vec::new();
    for project in &session.projects {
        let group = project.group.filter(|g| session.groups.iter().any(|x| x.id == *g));
        match ordered.iter_mut().find(|(g, _)| *g == group) {
            Some((_, list)) => list.push(project),
            None => ordered.push((group, vec![project])),
        }
    }
    let mut out: Vec<Candidate> = session
        .groups
        .iter()
        .map(|g| {
            let projects = ordered.iter().find(|(id, _)| *id == Some(g.id)).map(|(_, ps)| ps.as_slice()).unwrap_or(&[]);
            Candidate {
                kind: Kind::Group,
                goto: Goto::Group { id: g.id, first: projects.first().map(|p| p.id) },
                name: g.name.clone(),
                context: format!("{} project{}", projects.len(), if projects.len() == 1 { "" } else { "s" }),
                keys: vec![g.name.clone()],
                status: projects
                    .iter()
                    .filter_map(|p| p.loudest())
                    .max_by_key(|s| crate::model::status_rank(Some(s)))
                    .map(String::from),
            }
        })
        .collect();
    for (group, projects) in &ordered {
        let group_name = group.and_then(|g| session.groups.iter().find(|x| x.id == g)).map(|g| g.name.clone());
        for p in projects {
            out.push(Candidate {
                kind: Kind::Project,
                goto: Goto::Project(p.id),
                name: p.name.clone(),
                context: group_name.clone().unwrap_or_default(),
                keys: vec![p.name.clone()],
                status: p.loudest().map(String::from),
            });
            for w in &p.workspaces {
                let mut keys = vec![w.name.clone()];
                keys.extend(w.branch.clone().filter(|b| *b != w.name));
                out.push(Candidate {
                    kind: Kind::Workspace,
                    goto: Goto::Workspace(w.id),
                    name: if w.worktree { format!("⎇ {}", w.name) } else { w.name.clone() },
                    context: p.name.clone(),
                    keys: keys.clone(),
                    status: w.loudest().map(String::from),
                });
                for t in &w.tabs {
                    out.push(Candidate {
                        kind: Kind::Tab,
                        goto: Goto::Tab(t.id),
                        name: t.name.clone(),
                        context: format!("{} › {}", p.name, w.name),
                        keys: std::iter::once(t.name.clone()).chain(keys.iter().cloned()).collect(),
                        status: t.loudest().map(String::from),
                    });
                }
            }
        }
    }
    out
}

/// The candidates a query finds, best first: exact, prefix, a word's prefix, anywhere, then the
/// query's letters in order (fuzzy); ties by kind (group, project, workspace, tab), then the
/// sidebar's order. An empty query finds nothing, as in the TUI.
fn rank(candidates: Vec<Candidate>, query: &str) -> Vec<Ranked> {
    let query = query.trim();
    if query.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<(u8, usize, Ranked)> = candidates
        .into_iter()
        .enumerate()
        .filter_map(|(order, candidate)| {
            let best = candidate
                .keys
                .iter()
                .enumerate()
                .filter_map(|(i, key)| key_rank(key, query).map(|r| (r, i)))
                .min_by_key(|((rank, _), i)| (*rank, *i))?;
            let ((rank, ranges), key) = best;
            // Highlight on the name only when the name itself matched (the first key); a
            // worktree's `⎇ ` prefix shifts it.
            let shift = candidate.name.len() - candidate.keys[0].len().min(candidate.name.len());
            let highlight = if key == 0 && candidate.name.ends_with(candidate.keys[0].as_str()) {
                ranges.into_iter().map(|r| r.start + shift..r.end + shift).collect()
            } else {
                Vec::new()
            };
            Some((rank, order, Ranked { candidate, highlight }))
        })
        .collect();
    hits.sort_by_key(|(rank, order, r)| (*rank, r.candidate.kind, *order));
    hits.into_iter().map(|(_, _, r)| r).collect()
}

/// How well one key matches, and where.
fn key_rank(key: &str, query: &str) -> Option<(u8, Vec<Range<usize>>)> {
    let hay = fold(key);
    let needle: Vec<char> = fold(query).into_iter().map(|(_, c)| c).collect();
    if needle.is_empty() || needle.len() > hay.len() {
        return None;
    }
    let chars: Vec<char> = hay.iter().map(|(_, c)| *c).collect();
    let span = |start: usize, len: usize| hay[start].0.start..hay[start + len - 1].0.end;
    let starts: Vec<usize> =
        (0..=chars.len() - needle.len()).filter(|&i| chars[i..i + needle.len()] == needle[..]).collect();
    if let Some(&first) = starts.first() {
        let rank = if first == 0 && chars.len() == needle.len() {
            0
        } else if first == 0 {
            1
        } else if let Some(&word) = starts.iter().find(|&&i| !chars[i - 1].is_alphanumeric()) {
            return Some((2, vec![span(word, needle.len())]));
        } else {
            3
        };
        return Some((rank, vec![span(first, needle.len())]));
    }
    // The query's letters in order, each as early as it comes.
    let mut ranges = Vec::new();
    let mut next = 0;
    for (i, c) in chars.iter().enumerate() {
        if next < needle.len() && *c == needle[next] {
            ranges.push(hay[i].0.clone());
            next += 1;
        }
    }
    (next == needle.len()).then_some((4, ranges))
}

/// Each character of a text with its byte range, lower-cased one for one (a character whose
/// lower case is longer keeps its first), so matches map back onto the original text.
fn fold(text: &str) -> Vec<(Range<usize>, char)> {
    text.char_indices().map(|(i, c)| (i..i + c.len_utf8(), c.to_lowercase().next().unwrap_or(c))).collect()
}

/// Where a query first shows in a text, ignoring case, as a byte range of the text.
fn find(text: &str, query: &str) -> Option<Range<usize>> {
    let hay = fold(text);
    let needle: Vec<char> = fold(query.trim()).into_iter().map(|(_, c)| c).collect();
    if needle.is_empty() || needle.len() > hay.len() {
        return None;
    }
    (0..=hay.len() - needle.len())
        .find(|&i| hay[i..i + needle.len()].iter().map(|(_, c)| *c).eq(needle.iter().copied()))
        .map(|i| hay[i].0.start..hay[i + needle.len() - 1].0.end)
}

// ---------------------------------------------------------------------------------------------
// Sessions: what `cornercase search --json` prints.

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct Found {
    matches: Vec<Hit>,
    /// A match was left out over the limits (20 a pane, 300 in all).
    capped: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct Hit {
    pane: u64,
    /// `project › workspace › tab`.
    place: String,
    /// The agent or program.
    who: String,
    /// `prompt`, `reply` or `screen`.
    source: String,
    /// The line on the pane's screen, for a screen match.
    line: Option<u64>,
    text: String,
    /// When an agent's message was written (RFC 3339).
    written: Option<String>,
}

struct PaneHits<'a> {
    pane: u64,
    place: &'a str,
    who: &'a str,
    hits: Vec<&'a Hit>,
}

/// The matches gathered by pane, panes in the order their first match comes (the sidebar's),
/// matches in theirs (newest first).
fn by_pane(matches: &[Hit]) -> Vec<PaneHits<'_>> {
    let mut out: Vec<PaneHits> = Vec::new();
    for hit in matches {
        match out.iter_mut().find(|g| g.pane == hit.pane) {
            Some(group) => group.hits.push(hit),
            None => out.push(PaneHits { pane: hit.pane, place: &hit.place, who: &hit.who, hits: vec![hit] }),
        }
    }
    out
}

/// The list's row of the n-th match counted across groups, each group adding its header row.
fn row_of_match(groups: &[PaneHits], n: usize) -> usize {
    let mut row = 0;
    let mut seen = 0;
    for group in groups {
        row += 1;
        if n < seen + group.hits.len() {
            return row + n - seen;
        }
        row += group.hits.len();
        seen += group.hits.len();
    }
    row.saturating_sub(1)
}

/// Seconds since the epoch of an RFC 3339 time (`2026-10-23T17:48:54.840Z`, or with an offset).
fn parse_time(text: &str) -> Option<i64> {
    let (date, time) = text.split_once(['T', ' '])?;
    let mut d = date.splitn(3, '-').map(|p| p.parse::<i64>().ok());
    let (y, m, day) = (d.next()??, d.next()??, d.next()??);
    let (clock, offset) = match time.find(['Z', 'z', '+', '-']) {
        Some(i) => (&time[..i], &time[i..]),
        None => (time, "Z"),
    };
    let mut t = clock.split(':');
    let h: i64 = t.next()?.parse().ok()?;
    let min: i64 = t.next()?.parse().ok()?;
    let sec: i64 = t.next().map_or(Some(0.), |s| s.parse::<f64>().ok())? as i64;
    let offset = match offset.as_bytes().first() {
        Some(b'+' | b'-') => {
            let sign = if offset.starts_with('-') { -1 } else { 1 };
            let (oh, om) = offset[1..].split_once(':').unwrap_or((&offset[1..], "0"));
            sign * (oh.parse::<i64>().ok()? * 3600 + om.parse::<i64>().ok()? * 60)
        }
        _ => 0,
    };
    // Days from the civil date (Howard Hinnant's algorithm).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + h * 3600 + min * 60 + sec - offset)
}

/// How long ago a time was, shortly: `just now`, `5m ago`, `3h ago`, `2d ago`.
fn ago(at: i64) -> String {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    ago_from(at, now)
}

fn ago_from(at: i64, now: i64) -> String {
    let secs = (now - at).max(0);
    match secs {
        0..60 => "just now".into(),
        60..3600 => format!("{}m ago", secs / 60),
        3600..86_400 => format!("{}h ago", secs / 3600),
        _ => format!("{}d ago", secs / 86_400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        serde_json::from_value(serde_json::json!({
            "groups": [{"id": 1, "name": "Work"}],
            "projects": [
                {"id": 10, "name": "cornercase-terminal", "workspaces": [
                    {"id": 100, "name": "master", "tabs": [{"id": 1000, "name": "zsh"}, {"id": 1001, "name": "claude",
                        "panes": [{"id": 5, "status": "waiting"}]}]},
                    {"id": 101, "name": "fix-login", "branch": "fix/login", "worktree": true, "tabs": [{"id": 1002, "name": "codex"}]}
                ]},
                {"id": 11, "name": "gpui", "group": 1, "workspaces": [
                    {"id": 102, "name": "main", "tabs": [{"id": 1003, "name": "term"}]}
                ]}
            ]
        }))
        .unwrap()
    }

    fn names(query: &str) -> Vec<String> {
        rank(candidates(&session()), query).into_iter().map(|r| r.candidate.name).collect()
    }

    #[test]
    fn candidates_follow_the_sidebar() {
        let c = candidates(&session());
        assert_eq!(c[0].goto, Goto::Group { id: 1, first: Some(11) });
        assert_eq!(c[0].status, None);
        let kinds: Vec<Kind> = c.iter().map(|c| c.kind).collect();
        assert_eq!(kinds.iter().filter(|k| **k == Kind::Tab).count(), 4);
        let tab = c.iter().find(|c| c.goto == Goto::Tab(1001)).unwrap();
        assert_eq!(tab.context, "cornercase-terminal › master");
        assert_eq!(tab.status.as_deref(), Some("waiting"));
        let ws = c.iter().find(|c| c.goto == Goto::Workspace(101)).unwrap();
        assert_eq!(ws.name, "⎇ fix-login");
        assert_eq!(ws.keys, ["fix-login", "fix/login"]);
        // A tab is also found by its workspace's branch.
        assert!(c.iter().find(|c| c.goto == Goto::Tab(1002)).unwrap().keys.contains(&"fix/login".to_string()));
    }

    #[test]
    fn ranks_exact_then_prefix_then_word_then_anywhere_then_fuzzy() {
        assert_eq!(key_rank("zsh", "ZSH").unwrap().0, 0);
        assert_eq!(key_rank("master", "mas").unwrap().0, 1);
        assert_eq!(key_rank("fix/login", "log"), Some((2, vec![4..7])));
        assert_eq!(key_rank("cornercase", "case"), Some((3, vec![6..10])));
        assert_eq!(key_rank("cornercase-terminal", "cct").map(|r| r.0), Some(4));
        assert_eq!(key_rank("gpui", "xyz"), None);
        assert_eq!(key_rank("gpui", ""), None);
        assert_eq!(key_rank("ab", "abc"), None);
    }

    #[test]
    fn ties_go_by_kind_then_order() {
        // "c" starts cornercase-terminal (project), claude and codex (tabs).
        assert_eq!(names("c"), ["cornercase-terminal", "claude", "codex"]);
        assert_eq!(names("main")[0], "main");
        // Exact beats prefix whatever the kind.
        assert_eq!(names("zsh"), ["zsh"]);
        assert!(names("").is_empty());
        // A branch finds its workspace and its tabs, without a highlight on their names.
        let found = rank(candidates(&session()), "fix/login");
        assert_eq!(found[0].candidate.name, "⎇ fix-login");
        assert_eq!(found.len(), 2);
        assert!(found[1].highlight.is_empty());
    }

    #[test]
    fn highlights_land_on_the_name() {
        let found = rank(candidates(&session()), "login");
        let ws = &found[0];
        assert_eq!(ws.candidate.name, "⎇ fix-login");
        let r = ws.highlight[0].clone();
        assert_eq!(&ws.candidate.name[r], "login");
    }

    #[test]
    fn finds_ignoring_case_on_byte_ranges() {
        assert_eq!(find("Migration FAILED here", "failed"), Some(10..16));
        assert_eq!(find("…ça marche", "MARCHE"), Some(7..13));
        assert_eq!(find("abc", "zz"), None);
        assert_eq!(find("abc", "  "), None);
    }

    #[test]
    fn reads_and_groups_search_output() {
        let text = r#"{"capped": true, "matches": [
            {"line": null, "pane": 1, "place": "p › w › claude", "project": 4, "source": "reply", "tab": 2,
             "text": "a", "who": "claude", "workspace": 3, "written": "2026-10-23T17:48:54.840Z"},
            {"line": 40, "pane": 7, "place": "p › w › zsh", "source": "screen", "text": "b", "who": "zsh", "written": null},
            {"line": 12, "pane": 1, "place": "p › w › claude", "source": "screen", "text": "c", "who": "claude"}
        ], "future": 1}"#;
        let found: Found = serde_json::from_str(text).unwrap();
        assert!(found.capped);
        let groups = by_pane(&found.matches);
        assert_eq!(groups.iter().map(|g| g.pane).collect::<Vec<_>>(), [1, 7]);
        assert_eq!(groups[0].hits.iter().map(|h| h.text.as_str()).collect::<Vec<_>>(), ["a", "c"]);
        assert_eq!(groups[1].who, "zsh");
        assert_eq!(groups[1].hits[0].line, Some(40));
        // Rows: [header 1, a, c, header 7, b].
        assert_eq!(row_of_match(&groups, 0), 1);
        assert_eq!(row_of_match(&groups, 1), 2);
        assert_eq!(row_of_match(&groups, 2), 4);
    }

    #[test]
    fn reads_times_and_says_how_long_ago() {
        assert_eq!(parse_time("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_time("2026-10-23T17:48:54.840Z"), Some(1_792_777_734));
        assert_eq!(parse_time("2026-10-23T19:48:54+02:00"), Some(1_792_777_734));
        assert_eq!(parse_time("yesterday"), None);
        assert_eq!(ago_from(100, 130), "just now");
        assert_eq!(ago_from(0, 300), "5m ago");
        assert_eq!(ago_from(0, 7200), "2h ago");
        assert_eq!(ago_from(0, 3 * 86_400), "3d ago");
    }
}
