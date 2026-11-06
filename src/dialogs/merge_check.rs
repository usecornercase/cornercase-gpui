use gpui::{
    AnyElement, App, AppContext as _, ClickEvent, Context, Div, Entity, FocusHandle, FontWeight,
    InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render, Stateful,
    StatefulInteractiveElement as _, Styled as _, Subscription, Window, div, prelude::FluentBuilder as _, px,
};

use super::{ask_button, body_height, branch_line, close, frame, muted, show};
use crate::model::Session;
use crate::store::{self, Busy, Store};
use crate::theme::colours;
use crate::ui::{self, Tone};

mod answer;

use answer::{ASKED, Facts, Line, Reading, args, reading_of, shared_files_on};

const INTRO: &str = "Checked in a scratch copy, uncommitted work included.";

struct MergeCheckDialog {
    workspace: u64,
    focus: FocusHandle,
    reading: Reading,
    shared_on: Option<bool>,
    _store: Subscription,
}

struct Shown {
    name: String,
    lines: Vec<Line>,
    ask: Option<String>,
}

impl Shown {
    fn take(session: &Session, workspace: u64, reading: &Reading, shared_on: Option<bool>) -> Option<Self> {
        let (project, ws) = session.workspace(workspace)?;
        let name = |id: u64| session.workspace_name(id);
        let facts = Facts { project: &project.name, shared: &ws.shared, shared_on, name: &name };
        Some(Self { name: ws.name.clone(), lines: reading.lines(&facts), ask: reading.ask_text(&name) })
    }
}

fn gone(session: Option<&Session>, workspace: u64, removing: bool) -> bool {
    removing || session.is_none_or(|s| s.workspace(workspace).is_none())
}

pub(crate) fn merge_check(workspace: u64, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| MergeCheckDialog::new(workspace, cx));
    let focus = view.read(cx).focus.clone();
    show(view.into(), &focus, window, cx);
}

fn read_setting(cx: &mut Context<MergeCheckDialog>) {
    store::run_json(cx, store::args(&["config", "--json"]), |this, config, cx| {
        this.shared_on = config.ok().as_ref().and_then(shared_files_on);
        cx.notify();
    });
}

impl MergeCheckDialog {
    fn new(workspace: u64, cx: &mut Context<Self>) -> Self {
        let store = Store::get(cx);
        let follow = cx.observe(&store, |this: &mut Self, store, cx| this.follow(&store, cx));
        let focus = cx.focus_handle();
        let mut dialog = Self { workspace, focus, reading: Reading::Checking(None), shared_on: None, _store: follow };
        dialog.check(cx);
        read_setting(cx);
        dialog
    }

    fn follow(&mut self, store: &Entity<Store>, cx: &mut Context<Self>) {
        let s = store.read(cx);
        if s.dialog.as_ref().is_none_or(|d| d.entity_id() != cx.entity_id()) {
            return;
        }
        if gone(s.session.as_ref(), self.workspace, s.removing(self.workspace)) {
            store.update(cx, Store::close_dialog);
        }
        cx.notify();
    }

    fn check(&mut self, cx: &mut Context<Self>) {
        let name = Store::get(cx).read(cx).session.as_ref().map(|s| s.workspace_name(self.workspace));
        let busy = Busy::new(
            format!("merge-check:{}", self.workspace),
            format!("Checking how {} merges…", name.unwrap_or_default()),
        );
        self.reading = self.reading.again();
        store::run_partial_busy(cx, busy, args(self.workspace), |this, result, cx| {
            this.reading = reading_of(result);
            cx.notify();
        });
        cx.notify();
    }

    fn again(&mut self, cx: &mut Context<Self>) {
        if !self.reading.checking() {
            self.check(cx);
        }
    }

    fn buttons(&self, shown: &Shown, cx: &mut Context<Self>) -> Div {
        div()
            .flex_1()
            .flex()
            .items_center()
            .gap(px(8.))
            .children(shown.ask.clone().map(|text| ask_button(self.workspace, text, ASKED.to_string())))
            .child(div().flex_1())
            .child(ui::button("merge-check-done", "Done").on_click(|_: &ClickEvent, _, cx| close(cx)))
            .child(self.again_button(cx))
    }

    fn again_button(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        if self.reading.checking() {
            return ui::busy_button("merge-check-again", "Checking…", Tone::Primary);
        }
        ui::primary_button("merge-check-again", "Check again")
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.again(cx)))
    }
}

impl Render for MergeCheckDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let shown = Store::get(cx)
            .read(cx)
            .session
            .as_ref()
            .and_then(|s| Shown::take(s, self.workspace, &self.reading, self.shared_on));
        let height = body_height(f32::from(window.viewport_size().height));
        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                if e.keystroke.key == "enter" {
                    this.again(cx);
                }
            }))
            .children(
                shown.map(|shown| frame("Merge check", body(&shown, height), self.buttons(&shown, cx)).w(px(540.))),
            )
    }
}

fn body(shown: &Shown, height: f32) -> impl IntoElement {
    div()
        .id("merge-check-body")
        .max_h(px(height))
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(branch_line(shown.name.clone()))
        .child(muted(INTRO))
        .children(shown.lines.iter().enumerate().map(|(at, line)| drawn(at, line)))
}

fn drawn(at: usize, line: &Line) -> AnyElement {
    match line {
        Line::Heading(text, note) => heading(text, note.as_deref()).into_any_element(),
        Line::File(path) => file_line(path).into_any_element(),
        Line::Checking => ui::busy(("merge-check-checking", at), "Checking…").into_any_element(),
        Line::Clean => ui::done_line("Merges cleanly").into_any_element(),
        Line::Conflicts(text) => ui::warning_line(text.clone()).into_any_element(),
        Line::Conflict(path, kind) => conflict_line(path, kind).into_any_element(),
        Line::Failed(why) => ui::error_line(why.clone()).into_any_element(),
        Line::Note(text) => muted(text.clone()).into_any_element(),
        Line::Apart(text) => muted(text.clone()).pt(px(10.)).into_any_element(),
    }
}

fn heading(text: &str, note: Option<&str>) -> Div {
    div()
        .flex()
        .items_baseline()
        .gap(px(6.))
        .pt(px(10.))
        .text_size(px(13.))
        .child(
            div()
                .flex()
                .min_w_0()
                .font_weight(FontWeight::MEDIUM)
                .text_color(colours::text())
                .child(ui::one_line(text.to_string())),
        )
        .when_some(note, |d, note| d.child(muted(note.to_string()).flex_none()))
}

fn file_line(path: &str) -> Div {
    div().pl(px(18.)).text_size(px(12.)).text_color(colours::muted()).child(ui::one_line(path.to_string()))
}

fn conflict_line(path: &str, kind: &str) -> Div {
    div()
        .flex()
        .items_baseline()
        .gap(px(8.))
        .pl(px(18.))
        .text_size(px(12.))
        .text_color(colours::soft())
        .child(div().flex_1().min_w_0().child(ui::one_line(path.to_string())))
        .child(div().flex_none().text_color(colours::faint()).child(kind.to_string()))
}

#[cfg(test)]
mod tests;
