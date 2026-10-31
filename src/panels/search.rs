mod candidates;
mod found;
mod names;

use std::ops::Range;
use std::time::Duration;

use gpui::{
    AnyElement, AnyView, App, AppContext as _, ClickEvent, Context, Div, Entity, Focusable as _, FontWeight,
    HighlightStyle, Hsla, InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render, Rgba,
    ScrollHandle, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _,
    px,
};

use crate::panels::PanelKind;
use crate::panels::queue::open_pane;
use crate::store::{self, Store, args};
use crate::theme::{self, colours, radius};
use crate::ui::{self, Icon, OneLine, TextField, TextFieldEvent};
use candidates::candidates;
use found::{Found, Hit, PaneHits, by_pane, row_of_match};
use names::{Goto, Kind, Ranked, context_top, find, listing_status, rank, shown_row};

const MAX_NAMES: usize = 200;
const KIND_WIDTH: f32 = 92.;
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
    selected: usize,
    scroll: ScrollHandle,
    reveal: Option<usize>,
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
        let field = cx.new(|cx| TextField::new("Search…", cx).clears_on_escape());
        cx.subscribe_in(&field, window, |this, _, event, _, cx| match event {
            TextFieldEvent::Changed => this.changed(cx),
            TextFieldEvent::Submit => this.submit(cx),
            TextFieldEvent::Cancel => {}
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
        let mut panel = Self {
            field,
            mode: Mode::Names,
            selected: 0,
            scroll: ScrollHandle::new(),
            reveal: None,
            open: true,
            sessions: Sessions::default(),
        };
        panel.select_first(cx);
        panel
    }

    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.field.update(cx, TextField::clear);
        self.set_mode(Mode::Names, cx);
        window.focus(&self.field.read(cx).focus_handle(cx));
    }

    fn query(&self, cx: &App) -> String {
        self.field.read(cx).text().trim().to_string()
    }

    fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        self.mode = mode;
        self.select_first(cx);
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

    fn select_first(&mut self, cx: &App) {
        let (selected, top) = self.first_selection(cx);
        self.selected = selected;
        self.reveal = Some(top);
    }

    fn reveal_selection(&mut self, window: &Window) {
        let Some(top) = self.reveal else { return };
        if self.scroll.bounds().size.height > px(0.) {
            self.reveal = None;
        } else {
            window.request_animation_frame();
        }
        self.scroll.scroll_to_top_of_item(top);
    }

    fn first_selection(&self, cx: &App) -> (usize, usize) {
        if self.mode != Mode::Names || !self.query(cx).is_empty() {
            return (0, 0);
        }
        let shown = Store::get(cx).read(cx).session.as_ref().map(crate::model::Session::shown).unwrap_or_default();
        let (names, _) = self.names(cx);
        let selected = shown_row(&names, shown).unwrap_or(0);
        (selected, context_top(&names, selected))
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        self.select_first(cx);
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
        let (names, _) = self.names(cx);
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
            reveal(hit.reveal_args(&self.sessions.query), cx);
        }
    }

    fn names(&self, cx: &App) -> (Vec<Ranked>, usize) {
        let store = Store::get(cx);
        let Some(session) = &store.read(cx).session else { return (Vec::new(), 0) };
        let mut ranked = rank(candidates(session), &self.query(cx));
        let total = ranked.len();
        ranked.truncate(MAX_NAMES);
        (ranked, total)
    }

    fn count(&self, cx: &App) -> usize {
        match self.mode {
            Mode::Names => self.names(cx).0.len(),
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

    fn name_rows(&mut self, names: Vec<Ranked>, cx: &mut Context<Self>) -> Vec<AnyElement> {
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
                let row = MatchRow { index, selected: index == self.selected };
                rows.push(row.render(hit, &self.sessions.query, cx));
                index += 1;
            }
        }
        rows
    }

    fn names_and_status(&mut self, cx: &mut Context<Self>) -> (Vec<AnyElement>, String) {
        let (names, total) = self.names(cx);
        let status = listing_status(&self.query(cx), names.len(), total);
        (self.name_rows(names, cx), status)
    }

    fn sessions_and_status(&mut self, cx: &mut Context<Self>) -> (Vec<AnyElement>, String) {
        let query = self.query(cx);
        let status = if query.is_empty() {
            "Type to find what agents were told and answered, and what every pane printed".into()
        } else {
            self.sessions_status(&query)
        };
        (self.session_rows(cx), status)
    }

    fn sessions_status(&self, query: &str) -> String {
        if let Some(error) = &self.sessions.error {
            return error.clone();
        }
        match &self.sessions.found {
            _ if self.sessions.searching => "Searching…".into(),
            Some(found) if found.matches.is_empty() => format!("Nobody said “{query}”"),
            Some(found) => found_status(found),
            None => "Searching…".into(),
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

    fn status_bar(&self, status: String) -> Div {
        let sessions = self.mode == Mode::Sessions;
        let searching = sessions && self.sessions.searching;
        div().flex_none().flex().items_center().justify_between().gap(px(12.)).px(px(18.)).pt(px(8.)).pb(px(6.)).child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .text_size(px(12.))
                .text_color(if searching { colours::working() } else { colours::muted() })
                .when(self.sessions.error.is_some() && sessions, |d| d.text_color(colours::failed()))
                .when(searching, |d| d.child(ui::spinner("search-spinner", 12., colours::working())))
                .child(status),
        )
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

fn name_row(i: usize, selected: bool, hit: Ranked, cx: &mut Context<SearchPanel>) -> AnyElement {
    let Ranked { candidate: c, marks } = hit;
    let goto = c.goto;
    let side = |colour: Rgba| div().text_size(px(12.)).text_color(colour);
    ui::row(SharedString::from(format!("name-{i}")), selected)
        .child(kind_chip(c.kind))
        .child(
            div()
                .flex_none()
                .max_w(gpui::relative(0.5))
                .text_color(colours::text())
                .child(highlighted(c.name, marks.name)),
        )
        .child(side(colours::muted()).flex_1().min_w(px(0.)).child(highlighted(c.detail, marks.detail)))
        .child(side(colours::faint()).flex_none().max_w(gpui::relative(0.4)).child(highlighted(c.place, marks.place)))
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
        .px(px(10.))
        .py(px(5.))
        .rounded(px(radius::CONTROL))
        .cursor_pointer()
        .hover(|s| s.bg(colours::hover()))
        .child(ui::badge(status.as_deref()))
        .child(div().flex_1().font_weight(FontWeight::SEMIBOLD).child(ui::one_line(group.place.to_string())))
        .child(chip(group.who.to_string(), colours::muted()))
        .child(ui::dim(format!("pane {pane} · {count} match{}", if count == 1 { "" } else { "es" })))
        .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| open_pane(pane, cx)))
        .into_any_element()
}

struct MatchRow {
    index: usize,
    selected: bool,
}

impl MatchRow {
    fn render(&self, hit: &Hit, query: &str, cx: &mut Context<SearchPanel>) -> AnyElement {
        let command = hit.reveal_args(query);
        let range = find(&hit.text, query);
        ui::row(SharedString::from(format!("match-{}", self.index)), self.selected)
            .ml(px(22.))
            .child(source_chip(&hit.source))
            .child(
                div()
                    .flex_1()
                    .text_size(px(12.))
                    .text_color(colours::muted())
                    .child(highlighted(hit.text.clone(), range.into_iter().collect())),
            )
            .child(ui::dim(hit.when()))
            .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| reveal(command.clone(), cx)))
            .into_any_element()
    }
}

impl Render for SearchPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (rows, status) = match self.mode {
            Mode::Names => self.names_and_status(cx),
            Mode::Sessions => self.sessions_and_status(cx),
        };
        self.reveal_selection(window);
        div()
            .size_full()
            .flex()
            .flex_col()
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| this.key(e, cx)))
            .child(self.top_bar(window, cx))
            .child(self.status_bar(status))
            .child(self.results(rows))
            .child(footer())
    }
}

pub(crate) fn build(window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|cx| SearchPanel::new(window, cx)).into()
}

fn reveal(command: Vec<String>, cx: &mut App) {
    Store::get(cx).update(cx, |s, cx| {
        s.act(command);
        s.close_sheet(cx);
    });
}

fn go(goto: Goto, cx: &mut App) {
    Store::get(cx).update(cx, |s, cx| {
        if let Goto::Group { id, .. } = goto {
            s.cmd(args(&["expand", "--group", &id.to_string()]));
        }
        if let Some((kind, id)) = goto.focus() {
            s.focus(kind, id);
        }
        s.close_sheet(cx);
    });
}

fn chip_box(colour: impl Into<Hsla>) -> Div {
    div().flex_none().px(px(7.)).rounded(px(radius::CHIP)).bg(colours::sidebar()).text_size(px(11.)).text_color(colour)
}

fn chip(text: impl Into<SharedString>, colour: Rgba) -> Div {
    chip_box(colour).child(text.into())
}

fn kind_look(kind: Kind) -> (&'static str, Icon, Hsla) {
    match kind {
        Kind::Group => ("group", Icon::Files, colours::waiting().into()),
        Kind::Project => ("project", Icon::FolderOpen, colours::accent().into()),
        Kind::Workspace => ("workspace", Icon::Branch, colours::done().into()),
        Kind::Agent => ("agent", Icon::Agent, theme::hsla(theme::palette(5))),
        Kind::Tab => ("tab", Icon::Terminal, colours::muted().into()),
    }
}

fn kind_chip(kind: Kind) -> Div {
    let (label, glyph, colour) = kind_look(kind);
    let chip = chip_box(colour).flex().items_center().gap(px(4.)).child(ui::icon(glyph, 11.).text_color(colour));
    div().w(px(KIND_WIDTH)).flex_none().child(chip.child(label))
}

fn source_chip(source: &str) -> Div {
    let colour = match source {
        "prompt" => colours::accent(),
        "reply" => colours::done(),
        _ => colours::faint(),
    };
    div().w(px(56.)).flex_none().child(chip(source.to_string(), colour))
}

fn highlighted(text: impl Into<SharedString>, ranges: Vec<Range<usize>>) -> OneLine {
    let style = HighlightStyle {
        color: Some(colours::accent().into()),
        font_weight: Some(FontWeight::BOLD),
        ..Default::default()
    };
    ui::one_line(text).highlights(ranges.into_iter().map(|r| (r, style)))
}

#[cfg(test)]
mod tests;
