use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    AnyElement, ClickEvent, Context, Div, FontWeight, IntoElement, ListOffset, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, WeakEntity, div, list, prelude::FluentBuilder as _, px,
};

use super::data::{self, Issue, LIST_CAP, SourceInfo};
use super::{
    IssueId, IssuesPanel, ListState, Tab, blank, error_note, label_chip, state_chip, state_icon, virtual_list,
};
use crate::theme::{FONT_FAMILY, colours};
use crate::ui;

const LABELS_SHOWN: usize = 2;

pub(super) struct Rows {
    list: gpui::ListState,
    issues: Rc<Vec<Issue>>,
    capped: bool,
    width: f32,
}

impl Rows {
    pub(super) fn new() -> Self {
        Self { list: virtual_list(0).measure_all(), issues: Rc::default(), capped: false, width: 0. }
    }

    pub(super) fn sync(&mut self, issues: Vec<Issue>, capped: bool, width: f32) {
        if *self.issues == issues && self.capped == capped && self.width.total_cmp(&width).is_eq() {
            return;
        }
        let top = self.list.logical_scroll_top();
        self.list.reset(issues.len() + usize::from(capped));
        self.list.scroll_to(top);
        self.issues = Rc::new(issues);
        self.capped = capped;
        self.width = width;
    }

    pub(super) fn to_top(&self) {
        self.list.scroll_to(ListOffset::default());
    }

    pub(super) fn reveal(&self, index: usize) {
        self.list.scroll_to_reveal_item(index);
    }
}

struct Shown {
    issues: Rc<Vec<Issue>>,
    capped: bool,
    selected: Option<IssueId>,
    names: Option<HashMap<String, String>>,
    now: i64,
}

impl Shown {
    fn count(&self) -> usize {
        self.issues.len() + usize::from(self.capped)
    }

    fn info(&self, index: usize, issue: &Issue) -> RowInfo {
        let source = self.names.as_ref().map(|n| n.get(&issue.source).cloned().unwrap_or_else(|| issue.source.clone()));
        RowInfo { index, selected: self.selected.as_ref() == Some(&issue.id()), source, now: self.now }
    }
}

struct RowInfo {
    index: usize,
    selected: bool,
    source: Option<String>,
    now: i64,
}

impl IssuesPanel {
    pub(super) fn render_list(&self, cx: &mut Context<Self>) -> Div {
        let notes = self.notes();
        div()
            .size_full()
            .flex()
            .flex_col()
            .children(self.list_status(self.rows.issues.len()))
            .when(!notes.is_empty(), |d| {
                d.child(div().flex_none().flex().flex_col().gap(px(6.)).px(px(8.)).py(px(6.)).children(notes))
            })
            .child(self.list_body(cx))
    }

    fn notes(&self) -> Vec<Div> {
        let Some(state) = self.current() else { return Vec::new() };
        let failures =
            state.sources.iter().filter_map(|l| Some(format!("{}: {}", self.source_name(&l.source), l.error.clone()?)));
        let error = state.error.clone().filter(|_| !state.issues.is_empty());
        failures.chain(error).map(error_note).collect()
    }

    fn list_status(&self, shown: usize) -> Option<Div> {
        let state = self.current().filter(|s| !s.issues.is_empty())?;
        let source = self.tab.source().and_then(|s| self.source(s));
        let account = source.map(SourceInfo::account_line).filter(|a| !a.is_empty());
        let count = data::count_line(shown, state.issues.len(), self.closed);
        Some(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap(px(8.))
                .px(px(18.))
                .pt(px(10.))
                .pb(px(6.))
                .text_size(px(11.5))
                .text_color(colours::muted())
                .child(div().flex_1().min_w_0().child(ui::one_line(count)))
                .children(account.map(|a| ui::dim(a).flex_none())),
        )
    }

    fn list_body(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(state) = self.current() else {
            return ui::loading("Loading…").into_any_element();
        };
        if state.issues.is_empty() {
            return empty_note(state).into_any_element();
        }
        if self.rows.issues.is_empty() {
            return blank("Nothing in this list matches the filter.").into_any_element();
        }
        let shown = Rc::new(self.shown_rows());
        let panel = cx.entity().downgrade();
        list(self.rows.list.clone(), move |index, _, _| row_item(&shown, index, &panel)).flex_1().into_any_element()
    }

    fn shown_rows(&self) -> Shown {
        let names = (self.tab == Tab::All).then(|| {
            let sources = self.sources.iter().flat_map(|s| &s.list);
            sources.map(|s| (s.source.clone(), s.name.clone())).collect()
        });
        Shown {
            issues: self.rows.issues.clone(),
            capped: self.rows.capped,
            selected: self.selected.as_ref().map(Issue::id),
            names,
            now: data::now(),
        }
    }
}

fn row_item(shown: &Shown, index: usize, panel: &WeakEntity<IssuesPanel>) -> AnyElement {
    let content = match shown.issues.get(index) {
        Some(issue) => issue_row(issue.clone(), &shown.info(index, issue), panel.clone()).into_any_element(),
        None => cap_note().into_any_element(),
    };
    div()
        .w_full()
        .px(px(8.))
        .when(index > 0, |d| d.pt(px(2.)))
        .when(index + 1 == shown.count(), |d| d.pb(px(8.)))
        .child(content)
        .into_any_element()
}

fn empty_note(state: &ListState) -> Div {
    match &state.error {
        Some(e) => div().p(px(8.)).child(error_note(e.clone())),
        None if state.loading => ui::loading("Asking the trackers…"),
        None => blank("No issues match these filters."),
    }
}

fn cap_note() -> Div {
    ui::dim(format!("The first {LIST_CAP}, newest first: trackers answer no more. Narrow the filters to see others."))
        .p(px(10.))
}

fn issue_row(issue: Issue, info: &RowInfo, panel: WeakEntity<IssuesPanel>) -> impl IntoElement + use<> {
    ui::row(("issue-row", info.index), info.selected)
        .items_start()
        .gap(px(10.))
        .py(px(8.))
        .child(state_icon(&issue, 14.).mt(px(2.)))
        .child(row_text(&issue, info))
        .on_click(move |_: &ClickEvent, _, cx| {
            let _ = panel.update(cx, |this, cx| this.select(issue.clone(), cx));
        })
}

fn row_text(issue: &Issue, info: &RowInfo) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(
            div()
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(if issue.closed { colours::muted() } else { colours::text() })
                .line_clamp(2)
                .text_ellipsis()
                .child(issue.title.clone()),
        )
        .child(meta_line(issue, info))
}

fn meta_line(issue: &Issue, info: &RowInfo) -> Div {
    let key_colour = if issue.closed { colours::faint() } else { colours::accent() };
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .text_size(px(11.))
        .child(div().flex_none().font_family(FONT_FAMILY).text_color(key_colour).child(issue.key.clone()))
        .children(issue.state_note().map(|state| state_chip(state, issue.closed)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_color(colours::faint())
                .child(ui::one_line(issue.meta(info.source.clone(), info.now))),
        )
        .children(labels(issue))
}

fn labels(issue: &Issue) -> Vec<Div> {
    let more = issue.labels.len().saturating_sub(LABELS_SHOWN);
    let shown = issue.labels.iter().take(LABELS_SHOWN).cloned().map(label_chip);
    shown.chain((more > 0).then(|| label_chip(format!("+{more}")))).collect()
}

#[cfg(test)]
mod tests;
