use gpui::{
    AnyElement, ClickEvent, Context, Div, FontWeight, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _, px,
};

use super::WorktreesPanel;
use super::actions::{checking, freeing};
use super::data::{Tone, Worktree};
use crate::store::Store;
use crate::theme::{colours, radius};
use crate::ui;

fn mark(text: String, tone: Tone) -> Div {
    let colour = match tone {
        Tone::Plain => colours::muted(),
        Tone::Warn => colours::waiting(),
        Tone::Good => colours::done(),
    };
    div().px(px(6.)).rounded(px(radius::CHIP)).bg(colours::sidebar()).text_size(px(11.)).text_color(colour).child(text)
}

pub(super) fn activity(worktree: &Worktree, store: &Store) -> Option<(&'static str, ui::Tone)> {
    if worktree.workspace.is_some_and(|workspace| store.removing(workspace)) {
        Some(("Removing…", ui::Tone::Danger))
    } else if store.busy(&checking(&worktree.path)) {
        Some(("Measuring builds…", ui::Tone::Plain))
    } else if store.busy(&freeing(&worktree.path)) {
        Some(("Freeing builds…", ui::Tone::Plain))
    } else {
        None
    }
}

fn heading(worktree: &Worktree, now: i64, measuring: bool) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .child(div().font_weight(FontWeight::SEMIBOLD).child(worktree.title()))
        .children(worktree.marks().into_iter().map(|(text, tone)| mark(text, tone)))
        .child(div().flex_1())
        .child(ui::dim(worktree.age(now)))
        .child(div().text_size(px(12.)).child(worktree.size_label(measuring)))
}

fn details(worktree: &Worktree) -> Div {
    let risks = (!worktree.safe && !worktree.risks.is_empty()).then(|| worktree.risks.join(" · "));
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(ui::dim(ui::home_relative(&worktree.path)))
        .children(risks.map(|r| div().text_size(px(12.)).text_color(colours::waiting()).child(r)))
        .children(worktree.error.clone().map(|e| div().text_size(px(12.)).text_color(colours::failed()).child(e)))
}

impl WorktreesPanel {
    fn row_buttons(worktree: &Worktree, busy: Option<(&'static str, ui::Tone)>, cx: &mut Context<Self>) -> Div {
        let id = |what: &str| SharedString::from(format!("{what}-{}", worktree.path));
        if let Some((label, tone)) = busy {
            return div().flex().child(ui::busy_button(id("busy"), label, tone));
        }
        let (freed, removed) = (worktree.clone(), worktree.clone());
        div()
            .flex()
            .gap(px(6.))
            .children(worktree.workspace.map(|ws| {
                ui::button(id("changes"), "Changes").on_click(move |_: &ClickEvent, _, cx| Self::open_changes(ws, cx))
            }))
            .when(worktree.builds() > 0, |d| {
                d.child(
                    ui::button(id("free"), "Free builds…")
                        .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| Self::check_free(freed.clone(), cx))),
                )
            })
            .when(worktree.removable(), |d| {
                d.child(ui::danger_button(id("remove"), "Remove…").on_click(cx.listener(
                    move |_, _: &ClickEvent, window, cx| Self::ask_remove(std::slice::from_ref(&removed), window, cx),
                )))
            })
    }

    pub(super) fn row(&self, worktree: &Worktree, now: i64, cx: &mut Context<Self>) -> AnyElement {
        let path = worktree.path.clone();
        let busy = activity(worktree, Store::get(cx).read(cx));
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
            .rounded(px(radius::CARD))
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
                    .child(Self::row_buttons(worktree, busy, cx)),
            )
            .into_any_element()
    }
}
