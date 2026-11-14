use gpui::{
    AnyElement, App, ClickEvent, Context, Div, Focusable as _, FontWeight, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use super::QueuePanel;
use super::data::{Entry, Group, ROUTINES_NOTE, Row, Section, clear_goes_in, sections};
use super::rows::Snapshot;
use crate::notify::tip;
use crate::theme::colours;
use crate::ui::{self, Icon, TextField, Tone};

const NOTHING_SCHEDULED: &str = "Nothing is scheduled. Schedule… starts an agent later: at a time, in a while, when \
                                 your limits reset, or again and again as a routine.";

fn heading(group: Group, count: usize, action: Option<AnyElement>) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .min_h(px(26.))
        .px(px(12.))
        .pt(px(14.))
        .pb(px(4.))
        .child(
            div()
                .text_size(px(11.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(colours::muted())
                .child(group.title().to_uppercase()),
        )
        .children((count > 0).then(|| div().text_size(px(11.)).text_color(colours::faint()).child(count.to_string())))
        .child(div().flex_1())
        .children(action)
}

fn nothing_scheduled() -> Div {
    div()
        .flex()
        .items_start()
        .gap(px(10.))
        .px(px(12.))
        .py(px(8.))
        .text_size(px(12.))
        .text_color(colours::muted())
        .child(ui::icon(Icon::Queue, 14.).text_color(colours::faint()).mt(px(1.)))
        .child(div().flex_1().min_w(px(0.)).child(NOTHING_SCHEDULED))
}

fn routines_note() -> Div {
    ui::dim(ROUTINES_NOTE).px(px(12.)).pb(px(6.))
}

fn clear_started(cx: &mut Context<QueuePanel>) -> AnyElement {
    ui::button("queue-clear-started", "Clear started")
        .px(px(10.))
        .py(px(2.))
        .tooltip(tip("Remove every started entry and finished run from the list: their agents keep running"))
        .on_click(cx.listener(|_, _: &ClickEvent, _, cx| QueuePanel::clear_started(cx)))
        .into_any_element()
}

impl QueuePanel {
    fn search_box(&self, window: &Window, cx: &App) -> Div {
        let field = self.filter.clone();
        let focused = field.read(cx).focus_handle(cx).is_focused(window);
        let clear = (!field.read(cx).text().is_empty()).then(|| {
            let cleared = field.clone();
            ui::icon_button("queue-filter-clear", "✕")
                .on_click(move |_: &ClickEvent, _, cx| cleared.update(cx, TextField::clear))
        });
        ui::search_field(field, focused, clear)
    }

    pub(super) fn toolbar(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(10.))
            .px(px(16.))
            .py(px(12.))
            .border_b_1()
            .border_color(colours::border())
            .child(self.search_box(window, cx).flex_1().min_w(px(0.)))
            .child(
                ui::icon_text_button("queue-new", Icon::Plus, "Schedule…", Tone::Primary)
                    .tooltip(tip("Schedule an agent to start later, or a routine that repeats"))
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.open_form(None, window, cx))),
            )
    }

    fn row_and_runs(&self, row: &Row, snapshot: &Snapshot, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut parts = vec![self.entry_row(row, snapshot, cx)];
        parts.extend(row.runs.iter().map(|run| self.run_row(run, snapshot, cx)));
        parts
    }

    fn section(&self, section: &Section, clears: bool, snapshot: &Snapshot, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let action = clears.then(|| clear_started(cx));
        let mut parts = vec![heading(section.group, section.rows.len(), action).into_any_element()];
        if section.rows.is_empty() {
            parts.push(nothing_scheduled().into_any_element());
        }
        if section.group == Group::Routines {
            parts.push(routines_note().into_any_element());
        }
        parts.extend(section.rows.iter().flat_map(|row| self.row_and_runs(row, snapshot, cx)));
        parts
    }

    pub(super) fn list(&self, entries: &[Entry], filter: &str, snapshot: &Snapshot, cx: &mut Context<Self>) -> Div {
        let sections = sections(entries, filter, snapshot.session.as_ref());
        let clear = clear_goes_in(&sections);
        let parts: Vec<AnyElement> = sections
            .iter()
            .flat_map(|section| self.section(section, clear == Some(section.group), snapshot, cx))
            .collect();
        div().flex().flex_col().gap(px(2.)).px(px(10.)).pb(px(16.)).children(parts)
    }
}
