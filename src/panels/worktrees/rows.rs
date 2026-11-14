use gpui::{
    AnyElement, ClickEvent, Context, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _, px,
};

use super::WorktreesPanel;
use super::actions::removing;
use super::data::{Tone, Worktree};
use super::frees::{MergedJob, checking, freeing, merged_job};
use super::removal::{Said, Style};
use crate::notify::tip;
use crate::store::Store;
use crate::theme::{colours, radius};
use crate::ui;

const RUN_ROLE: &str = "A role of a running flow: it goes with the run's workspace.";

fn mark(text: String, tone: Tone) -> Div {
    let colour = match tone {
        Tone::Plain => colours::muted(),
        Tone::Warn => colours::waiting(),
        Tone::Good => colours::done(),
    };
    div().px(px(6.)).rounded(px(radius::chip())).bg(colours::inset()).text_size(px(11.)).text_color(colour).child(text)
}

fn said_line((text, style): &Said) -> Div {
    let line = div().text_size(px(12.)).child(text.clone());
    match style {
        Style::Heading => line.mt(px(6.)).font_weight(FontWeight::SEMIBOLD).text_color(colours::text()),
        Style::Risky => line.mt(px(6.)).font_weight(FontWeight::SEMIBOLD).text_color(colours::waiting()),
        Style::Line => line.pl(px(10.)).text_color(colours::muted()),
        Style::Lost => line.pl(px(10.)).text_color(colours::failed()),
        Style::Note => line.mt(px(10.)).text_color(colours::muted()),
    }
}

pub(super) fn said_lines(said: &[Said]) -> AnyElement {
    div().flex().flex_col().gap(px(2.)).children(said.iter().map(said_line)).into_any_element()
}

fn name(worktree: &Worktree) -> Stateful<Div> {
    let title = worktree.title();
    div()
        .id(SharedString::from(format!("name-{}", worktree.path)))
        .min_w_0()
        .flex()
        .font_weight(FontWeight::SEMIBOLD)
        .tooltip(tip(title.clone()))
        .child(ui::one_line(title))
}

fn heading(worktree: &Worktree, now: i64, measuring: bool) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .child(name(worktree))
        .children(worktree.marks().into_iter().map(|(text, tone)| mark(text, tone)))
        .child(div().flex_1())
        .child(ui::dim(worktree.age(now)))
        .child(div().text_size(px(12.)).child(worktree.size_label(measuring)))
}

fn details(worktree: &Worktree) -> Div {
    let risks = (!worktree.safe && !worktree.risks.is_empty()).then(|| {
        let parts = worktree.risks.iter().cloned().map(ui::wrapped_fact).collect();
        ui::dotted(parts, Vec::new()).text_size(px(12.)).text_color(colours::waiting())
    });
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(ui::dim(ui::home_relative(&worktree.path)))
        .when(worktree.run_role, |d| d.child(ui::dim(RUN_ROLE)))
        .children(risks)
        .children(worktree.error.clone().map(|e| div().text_size(px(12.)).text_color(colours::failed()).child(e)))
}

impl WorktreesPanel {
    pub(super) fn activity(&self, worktree: &Worktree, store: &Store) -> Option<(&'static str, ui::Tone)> {
        let path = worktree.path.as_str();
        if store.busy(&removing(path)) || worktree.workspace.is_some_and(|workspace| store.removing(workspace)) {
            Some(("Removing…", ui::Tone::Danger))
        } else if self.asking.iter().any(|asked| asked == path) {
            Some(("Checking…", ui::Tone::Plain))
        } else if store.busy(&checking(path)) {
            Some(("Measuring builds…", ui::Tone::Plain))
        } else if store.busy(&freeing(path)) {
            Some(("Freeing builds…", ui::Tone::Plain))
        } else {
            None
        }
    }

    fn remove_button(&self, worktree: &Worktree, cx: &mut Context<Self>) -> AnyElement {
        let id = SharedString::from(format!("remove-{}", worktree.path));
        if !self.asking.is_empty() {
            return ui::resting_button(id, "Remove…", ui::Tone::Danger).into_any_element();
        }
        let path = worktree.path.clone();
        ui::danger_button(id, "Remove…")
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.check_remove(vec![path.clone()], cx)))
            .into_any_element()
    }

    fn free_button(worktree: &Worktree, frees_merged: bool, cx: &mut Context<Self>) -> AnyElement {
        let id = SharedString::from(format!("free-{}", worktree.path));
        if frees_merged {
            return ui::resting_button(id, "Free builds…", ui::Tone::Plain).into_any_element();
        }
        let freed = worktree.clone();
        ui::button(id, "Free builds…")
            .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| Self::check_free(freed.clone(), cx)))
            .into_any_element()
    }

    fn row_buttons(
        &self,
        worktree: &Worktree,
        busy: Option<(&'static str, ui::Tone)>,
        frees_merged: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let id = |what: &str| SharedString::from(format!("{what}-{}", worktree.path));
        if let Some((label, tone)) = busy {
            return div().flex().child(ui::busy_button(id("busy"), label, tone));
        }
        div()
            .flex()
            .gap(px(6.))
            .children(worktree.workspace.map(|ws| {
                ui::button(id("changes"), "Changes").on_click(move |_: &ClickEvent, _, cx| Self::open_changes(ws, cx))
            }))
            .when(worktree.builds() > 0, |d| d.child(Self::free_button(worktree, frees_merged, cx)))
            .when(worktree.removable(), |d| d.child(self.remove_button(worktree, cx)))
    }

    pub(super) fn row(&self, worktree: &Worktree, now: i64, cx: &mut Context<Self>) -> AnyElement {
        let path = worktree.path.clone();
        let store = Store::get(cx).read(cx);
        let busy = self.activity(worktree, store);
        let frees_merged = merged_job(|key| store.busy(key)) != MergedJob::Idle;
        let ticked = self.ticked.contains(&path) && busy.is_none();
        let tick = ui::checkbox(SharedString::from(format!("tick-{path}")), ticked, "");
        let tick = if worktree.removable() && busy.is_none() {
            tick.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle(path.clone(), cx)))
        } else {
            tick.opacity(0.3).cursor_default()
        };
        div()
            .flex()
            .items_start()
            .gap(px(10.))
            .p(px(12.))
            .rounded(px(radius::card()))
            .bg(colours::terminal())
            .border_1()
            .border_color(colours::border())
            .child(div().pt(px(2.)).child(tick))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(heading(worktree, now, self.measuring).when(busy.is_some(), |d| d.opacity(0.6)))
                    .child(details(worktree))
                    .child(self.row_buttons(worktree, busy, frees_merged, cx)),
            )
            .into_any_element()
    }
}
