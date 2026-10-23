mod found;
mod names;

use std::ops::Range;
use std::time::Duration;

use gpui::{
    AnyElement, AnyView, App, AppContext as _, ClickEvent, Context, Div, Entity, Focusable as _, FontWeight,
    HighlightStyle, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render, Rgba, ScrollHandle,
    SharedString, StatefulInteractiveElement as _, Styled as _, StyledText, Window, div, prelude::FluentBuilder as _,
    px,
};

use crate::panels::PanelKind;
use crate::panels::queue::open_pane;
use crate::store::{self, Store, args};
use crate::theme::colours;
use crate::ui::{self, TextField, TextFieldEvent};
use found::{Found, Hit, PaneHits, by_pane, row_of_match};
use names::{Goto, Kind, Ranked, candidates, find, rank};

const MAX_NAMES: usize = 200;
const DEBOUNCE: Duration = Duration::from_millis(220);
const SCROLL_NOTE: &str = "Opens the pane; scrolling to the match needs usecornercase/cornercase-terminal#467";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Mode {
    #[default]
    Names,
    Sessions,
}

pub(crate) struct SearchPanel {
    field: Entity<TextField>,
    mode: Mode,
    selected: usize,
    scroll: ScrollHandle,
    open: bool,
    sessions: Sessions,
}

#[derive(Default)]
struct Sessions {
    generation: u64,
    query: String,
    searching: bool,
    found: Option<Found>,
    error: Option<String>,
    enter: bool,
}

impl SearchPanel {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let field = cx.new(|cx| TextField::new("Search…", cx));
        cx.subscribe_in(&field, window, |this, _, event, _, cx| match event {
            TextFieldEvent::Changed => this.changed(cx),
            TextFieldEvent::Submit => this.submit(cx),
            TextFieldEvent::Cancel => Self::close(cx),
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

    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.field.update(cx, TextField::clear);
        self.set_mode(Mode::Names, cx);
        window.focus(&self.field.read(cx).focus_handle(cx));
    }

    fn close(cx: &mut App) {
        Store::get(cx).update(cx, Store::close_sheet);
    }

    fn query(&self, cx: &App) -> String {
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

    fn choose_mode(&mut self, mode: Mode, window: &mut Window, cx: &mut Context<Self>) {
        if self.mode != mode {
            self.set_mode(mode, cx);
        }
        window.focus(&self.field.read(cx).focus_handle(cx));
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        self.selected = 0;
        self.scroll.scroll_to_item(0);
        if self.mode == Mode::Sessions {
            self.schedule(cx);
        }
        cx.notify();
    }

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
            let _ = this.update(cx, |this, cx| this.search_if_current(&query, generation, cx));
        })
        .detach();
    }

    fn search_if_current(&mut self, query: &str, generation: u64, cx: &mut Context<Self>) {
        if self.sessions.generation == generation {
            self.search(query, generation, cx);
        }
    }

    fn search(&mut self, query: &str, generation: u64, cx: &mut Context<Self>) {
        query.clone_into(&mut self.sessions.query);
        store::run_json(cx, args(&["search", "--json", "--", query]), move |this, result, cx| {
            let found = result.and_then(|v| serde_json::from_value::<Found>(v).map_err(anyhow::Error::from));
            this.answered(generation, found, cx);
        });
    }

    fn answered(&mut self, generation: u64, found: anyhow::Result<Found>, cx: &mut Context<Self>) {
        if self.sessions.generation != generation {
            return;
        }
        self.sessions.searching = false;
        match found {
            Ok(found) => {
                self.sessions.error = None;
                self.sessions.found = Some(found);
                if std::mem::take(&mut self.sessions.enter) {
                    self.submit(cx);
                }
            }
            Err(e) => {
                self.sessions.found = None;
                self.sessions.error = Some(e.to_string());
            }
        }
        cx.notify();
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        match self.mode {
            Mode::Names => self.submit_name(cx),
            Mode::Sessions => self.submit_match(cx),
        }
    }

    fn submit_name(&self, cx: &mut Context<Self>) {
        let names = self.names(cx);
        if let Some(hit) = names.get(self.selected.min(names.len().saturating_sub(1))) {
            go(hit.candidate.goto, cx);
        }
    }

    fn submit_match(&mut self, cx: &mut Context<Self>) {
        let current = self.sessions.query == self.query(cx) && !self.sessions.searching;
        let Some(found) = self.sessions.found.as_ref().filter(|_| current) else {
            self.sessions.enter = !self.query(cx).is_empty();
            return;
        };
        if let Some(hit) = found.matches.get(self.selected) {
            open_pane(hit.pane, cx);
        }
    }

    fn names(&self, cx: &App) -> Vec<Ranked> {
        let store = Store::get(cx);
        let Some(session) = &store.read(cx).session else { return Vec::new() };
        let mut ranked = rank(candidates(session), &self.query(cx));
        ranked.truncate(MAX_NAMES);
        ranked
    }

    fn count(&self, cx: &App) -> usize {
        match self.mode {
            Mode::Names => self.names(cx).len(),
            Mode::Sessions => self.sessions.found.as_ref().map_or(0, |f| f.matches.len()),
        }
    }

    fn selected_row(&self) -> usize {
        match self.mode {
            Mode::Names => self.selected,
            Mode::Sessions => {
                self.sessions.found.as_ref().map_or(0, |f| row_of_match(&by_pane(&f.matches), self.selected))
            }
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
        self.scroll.scroll_to_item(self.selected_row());
        cx.notify();
    }

    fn name_rows(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let names = self.names(cx);
        self.selected = self.selected.min(names.len().saturating_sub(1));
        names.into_iter().enumerate().map(|(i, hit)| name_row(i, i == self.selected, hit, cx)).collect()
    }

    fn session_rows(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(found) = &self.sessions.found else { return Vec::new() };
        self.selected = self.selected.min(found.matches.len().saturating_sub(1));
        let mut rows = Vec::new();
        let mut index = 0;
        for group in by_pane(&found.matches) {
            rows.push(pane_header(&group, index == 0, cx));
            for hit in group.hits {
                let row = MatchRow { index, selected: index == self.selected, pane: group.pane };
                rows.push(row.render(hit, &self.sessions.query, cx));
                index += 1;
            }
        }
        rows
    }

    fn status(&self, cx: &App) -> (String, Option<String>) {
        let query = self.query(cx);
        match self.mode {
            Mode::Names if query.is_empty() => {
                ("Type to find a group, project, workspace or tab by its name or branch".into(), None)
            }
            Mode::Names => (self.names_status(&query, cx), None),
            Mode::Sessions if query.is_empty() => {
                ("Type to find what agents were told and answered, and what every pane printed".into(), None)
            }
            Mode::Sessions => self.sessions_status(&query),
        }
    }

    fn names_status(&self, query: &str, cx: &App) -> String {
        match self.names(cx).len() {
            0 => format!("Nothing is called “{query}”"),
            n => format!("{n} result{}", if n == 1 { "" } else { "s" }),
        }
    }

    fn sessions_status(&self, query: &str) -> (String, Option<String>) {
        if let Some(error) = &self.sessions.error {
            return (error.clone(), None);
        }
        match &self.sessions.found {
            _ if self.sessions.searching => ("Searching…".into(), None),
            Some(found) if found.matches.is_empty() => (format!("Nobody said “{query}”"), None),
            Some(found) => (found_status(found), Some(SCROLL_NOTE.to_string())),
            None => ("Searching…".into(), None),
        }
    }

    fn mode_segments(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let mode = self.mode;
        let segment = |id: &'static str, label: &'static str, which: Mode| {
            ui::segment(id, label, mode == which)
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.choose_mode(which, window, cx)))
                .into_any_element()
        };
        ui::segments(vec![
            segment("mode-names", "Names", Mode::Names),
            segment("mode-sessions", "In sessions", Mode::Sessions),
        ])
    }

    fn top_bar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let focused = self.field.read(cx).focus_handle(cx).is_focused(window);
        let prompt = div()
            .text_color(if focused { colours::accent() } else { colours::faint() })
            .child(if self.mode == Mode::Names { "⌕" } else { "❯" });
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(10.))
            .px(px(16.))
            .pt(px(14.))
            .child(div().flex_1().child(ui::input_box(
                div().flex().items_center().gap(px(8.)).child(prompt).child(div().flex_1().child(self.field.clone())),
                focused,
            )))
            .child(self.mode_segments(cx))
    }

    fn status_bar(&self, status: String, note: Option<String>) -> Div {
        let sessions = self.mode == Mode::Sessions;
        let searching = sessions && self.sessions.searching;
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
                    .when(self.sessions.error.is_some() && sessions, |d| d.text_color(colours::failed()))
                    .child(status),
            )
            .children(note.map(ui::dim))
    }

    fn results(&self, rows: Vec<AnyElement>) -> impl IntoElement + use<> {
        let searching = self.mode == Mode::Sessions && self.sessions.searching;
        div()
            .id("search-results")
            .flex_1()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .px(px(10.))
            .pb(px(10.))
            .opacity(if searching && !rows.is_empty() { 0.55 } else { 1. })
            .children(rows)
    }
}

fn found_status(found: &Found) -> String {
    let panes = by_pane(&found.matches).len();
    let n = found.matches.len();
    let capped = if found.capped { " (more were left out)" } else { "" };
    format!("{n} match{} in {panes} pane{}{capped}", if n == 1 { "" } else { "es" }, if panes == 1 { "" } else { "s" })
}

fn footer() -> impl IntoElement {
    div().flex_none().flex().gap(px(16.)).px(px(18.)).py(px(8.)).border_t_1().border_color(colours::border()).children(
        [("↑↓", "move"), ("↵", "go there"), ("⇥", "names or sessions"), ("esc", "close")].map(|(key, what)| {
            div()
                .flex()
                .gap(px(5.))
                .text_size(px(11.))
                .child(div().text_color(colours::muted()).child(key))
                .child(div().text_color(colours::faint()).child(what))
        }),
    )
}

fn ellipsis() -> Div {
    div().overflow_hidden().whitespace_nowrap().text_ellipsis()
}

fn name_row(i: usize, selected: bool, hit: Ranked, cx: &mut Context<SearchPanel>) -> AnyElement {
    let c = hit.candidate;
    let goto = c.goto;
    ui::row(SharedString::from(format!("name-{i}")), selected)
        .child(kind_chip(c.kind))
        .child(
            ellipsis()
                .flex_none()
                .max_w(gpui::relative(0.55))
                .text_color(colours::text())
                .child(highlighted(c.name, hit.highlight)),
        )
        .child(ellipsis().flex_1().text_size(px(12.)).text_color(colours::faint()).child(c.context))
        .child(ui::badge(c.status.as_deref()))
        .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| go(goto, cx)))
        .into_any_element()
}

fn pane_header(group: &PaneHits, first: bool, cx: &mut Context<SearchPanel>) -> AnyElement {
    let pane = group.pane;
    let status = Store::get(cx).read(cx).session.as_ref().and_then(|s| s.pane(pane)).and_then(|p| p.status.clone());
    let count = group.hits.len();
    div()
        .id(SharedString::from(format!("pane-{pane}")))
        .flex()
        .items_center()
        .gap(px(8.))
        .mt(px(if first { 2. } else { 10. }))
        .px(px(8.))
        .py(px(4.))
        .rounded(px(5.))
        .cursor_pointer()
        .hover(|s| s.bg(colours::hover()))
        .child(ui::badge(status.as_deref()))
        .child(ellipsis().flex_1().font_weight(FontWeight::SEMIBOLD).child(group.place.to_string()))
        .child(chip(group.who.to_string(), colours::muted()))
        .child(ui::dim(format!("pane {pane} · {count} match{}", if count == 1 { "" } else { "es" })))
        .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| open_pane(pane, cx)))
        .into_any_element()
}

struct MatchRow {
    index: usize,
    selected: bool,
    pane: u64,
}

impl MatchRow {
    fn render(&self, hit: &Hit, query: &str, cx: &mut Context<SearchPanel>) -> AnyElement {
        let pane = self.pane;
        let range = find(&hit.text, query);
        ui::row(SharedString::from(format!("match-{}", self.index)), self.selected)
            .ml(px(22.))
            .child(source_chip(&hit.source))
            .child(
                ellipsis()
                    .flex_1()
                    .text_size(px(12.))
                    .text_color(colours::muted())
                    .child(highlighted(hit.text.clone(), range.into_iter().collect())),
            )
            .child(ui::dim(hit.when()))
            .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| open_pane(pane, cx)))
            .into_any_element()
    }
}

impl Render for SearchPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = match self.mode {
            Mode::Names => self.name_rows(cx),
            Mode::Sessions => self.session_rows(cx),
        };
        let (status, note) = self.status(cx);
        div()
            .size_full()
            .flex()
            .flex_col()
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| this.key(e, cx)))
            .child(self.top_bar(window, cx))
            .child(self.status_bar(status, note))
            .child(self.results(rows))
            .child(footer())
    }
}

pub(crate) fn build(window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|cx| SearchPanel::new(window, cx)).into()
}

fn go(goto: Goto, cx: &mut App) {
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

fn chip(text: impl Into<SharedString>, colour: Rgba) -> Div {
    div()
        .flex_none()
        .px(px(6.))
        .rounded(px(4.))
        .bg(colours::sidebar())
        .text_size(px(11.))
        .text_color(colour)
        .child(text.into())
}

fn kind_chip(kind: Kind) -> Div {
    let (label, colour) = match kind {
        Kind::Group => ("group", colours::waiting()),
        Kind::Project => ("project", colours::accent()),
        Kind::Workspace => ("workspace", colours::done()),
        Kind::Tab => ("tab", colours::muted()),
    };
    div().w(px(76.)).flex_none().child(chip(label, colour))
}

fn source_chip(source: &str) -> Div {
    let colour = match source {
        "prompt" => colours::accent(),
        "reply" => colours::done(),
        _ => colours::faint(),
    };
    div().w(px(56.)).flex_none().child(chip(source.to_string(), colour))
}

fn highlighted(text: impl Into<SharedString>, ranges: Vec<Range<usize>>) -> StyledText {
    let style = HighlightStyle {
        color: Some(colours::accent().into()),
        font_weight: Some(FontWeight::BOLD),
        ..Default::default()
    };
    StyledText::new(text).with_highlights(ranges.into_iter().map(|r| (r, style)))
}

#[cfg(test)]
mod tests;
