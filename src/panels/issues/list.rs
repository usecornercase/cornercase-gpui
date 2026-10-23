use gpui::{
    AnyElement, ClickEvent, Context, Div, Focusable as _, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

use super::data::{self, Issue, LIST_CAP, SourceInfo, SourceState, Sources, Who};
use super::{IssuesPanel, ListState, Tab, capitalise, error_note, label_chip, state_chip};
use crate::theme::{FONT_FAMILY, colours};
use crate::ui;

struct Pick {
    index: usize,
    who: Who,
}

struct RowInfo {
    index: usize,
    selected: bool,
    source: Option<String>,
    now: i64,
}

impl IssuesPanel {
    pub(super) fn render_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let any = self.sources.as_ref().is_some_and(Sources::any_connected);
        let mut items: Vec<AnyElement> = vec![
            ui::segment("tab-all", "All", self.tab == Tab::All)
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.set_tab(Tab::All, cx)))
                .when(!any, |d| d.text_color(colours::faint()))
                .into_any_element(),
        ];
        for source in self.sources.as_ref().map(Sources::tabs).unwrap_or_default() {
            let selected = self.tab == Tab::One(source.source.clone());
            items.push(tab_segment(source, selected, cx));
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

    pub(super) fn render_filters(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
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
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_closed(cx))),
            );
        for (index, field) in fields.iter().enumerate() {
            row = row.child(self.who_button(index, field, &who[index], cx));
        }
        row
    }

    fn who_button(&self, index: usize, field: &str, who: &Who, cx: &mut Context<Self>) -> Stateful<Div> {
        let active = *who != Who::Anyone;
        let open = self.picker == Some(index);
        ui::button_coloured(
            ("issues-who", index),
            format!("{}: {} ▾", capitalise(field), who.label()),
            if active { colours::accent() } else { colours::muted() },
        )
        .when(active || open, |d| d.border_color(colours::accent()))
        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.toggle_picker(index, window, cx)))
    }

    pub(super) fn render_picker(
        &self,
        index: usize,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let current = self.who(&self.tab)[index].clone();
        let mut chips: Vec<AnyElement> = [Who::Anyone, Who::Me]
            .into_iter()
            .map(|who| {
                let selected = current == who;
                who_chip(
                    SharedString::from(format!("who-{}", who.label())),
                    who.label(),
                    selected,
                    Pick { index, who },
                    cx,
                )
            })
            .collect();
        let note = self.people_chips(index, &current, &mut chips, cx);
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

    fn people_chips(
        &self,
        index: usize,
        current: &Who,
        chips: &mut Vec<AnyElement>,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        let Some(source) = self.tab.source() else {
            return Some("Pick a tracker's tab to filter by someone in particular.".to_string());
        };
        let people = match self.people_lists.get(source) {
            None => return Some("Asking the tracker for its people…".into()),
            Some(Err(e)) => return Some(e.clone()),
            Some(Ok(people)) => people,
        };
        let matching = people.matching(&self.picker_search.read(cx).text());
        for (i, person) in matching.iter().take(80).enumerate() {
            let who = person.who();
            let selected = *current == who;
            chips.push(who_chip(
                SharedString::from(format!("who-p-{i}")),
                person.label(),
                selected,
                Pick { index, who },
                cx,
            ));
        }
        matching.is_empty().then(|| "Nobody matches.".into())
    }

    pub(super) fn render_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let list = div().id("issues-list").size_full().overflow_y_scroll().flex().flex_col().gap(px(2.)).p(px(6.));
        let list = list.children(self.failure_notes());
        let Some(state) = self.current() else {
            return list.child(ui::note("Loading…")).into_any_element();
        };
        if state.issues.is_empty() {
            return list.child(empty_note(state)).into_any_element();
        }
        let list = list.children(state.error.clone().map(error_note));
        let shown = self.shown(cx);
        if shown.is_empty() {
            return list.child(ui::note("Nothing in this list matches the search.")).into_any_element();
        }
        list.children(self.rows(shown, cx))
            .when(state.issues.len() >= LIST_CAP, |d| d.child(cap_note()))
            .into_any_element()
    }

    fn failure_notes(&self) -> Vec<Div> {
        let Some(state) = self.current() else { return Vec::new() };
        state
            .sources
            .iter()
            .filter_map(|l| Some(error_note(format!("{}: {}", self.source_name(&l.source), l.error.clone()?))))
            .collect()
    }

    fn rows(&self, shown: Vec<Issue>, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let now = data::now();
        let all = self.tab == Tab::All;
        let selected = self.selected.as_ref().map(Issue::id);
        shown
            .into_iter()
            .enumerate()
            .map(|(index, issue)| {
                let info = RowInfo {
                    index,
                    selected: selected.as_ref() == Some(&issue.id()),
                    source: all.then(|| self.source_name(&issue.source)),
                    now,
                };
                issue_row(issue, info, cx).into_any_element()
            })
            .collect()
    }
}

fn tab_segment(source: &SourceInfo, selected: bool, cx: &mut Context<IssuesPanel>) -> AnyElement {
    let tab = Tab::One(source.source.clone());
    let (dot, colour) = match source.state {
        SourceState::Connected => ("●", colours::done()),
        SourceState::Failing => ("!", colours::failed()),
        _ => ("○", colours::faint()),
    };
    ui::segment(SharedString::from(format!("tab-{}", source.source)), source.name.clone(), selected)
        .flex()
        .items_center()
        .gap(px(5.))
        .when(!source.connected() && !selected, |d| d.text_color(colours::faint()))
        .child(div().text_size(px(8.)).text_color(colour).child(dot))
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_tab(tab.clone(), cx)))
        .into_any_element()
}

fn who_chip(id: SharedString, label: String, selected: bool, pick: Pick, cx: &mut Context<IssuesPanel>) -> AnyElement {
    let Pick { index, who } = pick;
    ui::segment(id, label, selected)
        .border_1()
        .border_color(if selected { colours::accent() } else { colours::border() })
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_who(index, who.clone(), cx)))
        .into_any_element()
}

fn empty_note(state: &ListState) -> Div {
    match &state.error {
        Some(e) => error_note(e.clone()),
        None => ui::note(if state.loading { "Asking the trackers…" } else { "No issues match these filters." }),
    }
}

fn cap_note() -> Div {
    ui::dim(format!("The first {LIST_CAP}, newest first: trackers answer no more. Narrow the filters to see others."))
        .p(px(8.))
}

fn issue_row(issue: Issue, info: RowInfo, cx: &mut Context<IssuesPanel>) -> impl IntoElement + use<> {
    let selected = info.selected;
    let meta = issue.meta(info.source, info.now);
    div()
        .id(("issue-row", info.index))
        .flex()
        .flex_col()
        .gap(px(2.))
        .px(px(10.))
        .py(px(6.))
        .rounded(px(6.))
        .cursor_pointer()
        .when(selected, |d| d.bg(colours::selected()))
        .when(!selected, |d| d.hover(|s| s.bg(colours::hover())))
        .child(key_line(&issue))
        .child(
            div()
                .text_size(px(13.))
                .text_color(if issue.closed { colours::muted() } else { colours::text() })
                .line_clamp(2)
                .child(issue.title.clone()),
        )
        .child(ui::dim(meta).whitespace_nowrap().overflow_hidden().text_ellipsis())
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.select(issue.clone(), cx)))
}

fn key_line(issue: &Issue) -> Div {
    let key_colour = if issue.closed { colours::faint() } else { colours::accent() };
    let state = (!issue.state.is_empty() && issue.state != "open").then(|| state_chip(&issue.state, issue.closed));
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
        .children(state)
        .child(div().flex_1())
        .children(issue.labels.iter().take(3).cloned().map(label_chip))
}
