use std::time::{Duration, Instant};

use gpui::{
    App, AppContext as _, ClickEvent, Context, Div, Entity, FocusHandle, Hsla, InteractiveElement as _, IntoElement,
    KeyDownEvent, ParentElement as _, Render, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _,
    Subscription, Task, Window, div, prelude::FluentBuilder as _, px, relative,
};

use super::{body_height, close, frame, show};
use crate::cli;
use crate::model::{Breakdown, ContextAnswer, PaneInfo, Session, Tab};
use crate::store::{self, Busy, Store};
use crate::theme::{self, colours};
use crate::ui::{self, Icon, Tone, glass};

mod lists;
mod read;

use lists::{Fill, Row, Section, Slice};
use read::{Reads, TOO_OLD, outcome_of};

pub(crate) const VIEW_CONTEXT: &str = "View context";

const PALETTE: [u8; 9] = [141, 175, 110, 180, 139, 73, 174, 146, 104];
const AGE_REDRAW: Duration = Duration::from_secs(30);
const BAR_HEIGHT: f32 = 10.;
const SWATCH: f32 = 9.;
const TOKENS_WIDTH: f32 = 52.;
const SHARE_WIDTH: f32 = 44.;
const REFRESH_WIDTH: f32 = 100.;
const GIVES_WAY: f32 = 1_000.;
const NOTHING_SPENT: &str = "Nothing is typed into the agent and no tokens are spent. \
                             While it reads, the agent's MCP servers start a second time.";

pub(crate) fn context_choices(tab: &Tab) -> Vec<(u64, String)> {
    let agents: Vec<&PaneInfo> = tab.panes.iter().filter(|p| p.agent.is_some()).collect();
    if let [only] = agents[..] {
        return vec![(only.id, VIEW_CONTEXT.to_string())];
    }
    let label = |(at, pane): (usize, &&PaneInfo)| {
        let name = pane.agent.as_deref().unwrap_or_default();
        let before = agents[..at].iter().filter(|p| p.agent == pane.agent).count();
        let count = if before == 0 { String::new() } else { format!(" {}", before + 1) };
        (pane.id, format!("{VIEW_CONTEXT} · {name}{count}"))
    };
    agents.iter().enumerate().map(label).collect()
}

pub(crate) struct ContextDialog {
    pane: u64,
    focus: FocusHandle,
    reads: Reads,
    _store: Subscription,
    _age: Task<()>,
}

pub(crate) fn view_context(pane: u64, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| {
        let store = Store::get(cx);
        let follow = cx.observe(&store, |this: &mut ContextDialog, store, cx| this.follow(&store, cx));
        let age = age_redraws(cx);
        ContextDialog { pane, focus: cx.focus_handle(), reads: Reads::default(), _store: follow, _age: age }
    });
    let focus = view.read(cx).focus.clone();
    show(view.clone().into(), &focus, window, cx);
    view.update(cx, ContextDialog::ask_cached);
}

fn age_redraws(cx: &mut Context<ContextDialog>) -> Task<()> {
    cx.spawn(async move |this, cx| {
        loop {
            cx.background_executor().timer(AGE_REDRAW).await;
            if this.update(cx, |_, cx| cx.notify()).is_err() {
                break;
            }
        }
    })
}

fn agent_gone(session: Option<&Session>, pane: u64) -> bool {
    session.is_some_and(|s| s.pane(pane).is_none_or(|p| p.agent.is_none()))
}

fn shown(cx: &Context<ContextDialog>) -> bool {
    Store::get(cx).read(cx).dialog.as_ref().is_some_and(|d| d.entity_id() == cx.entity_id())
}

impl ContextDialog {
    fn follow(&mut self, store: &Entity<Store>, cx: &mut Context<Self>) {
        if shown(cx) && agent_gone(store.read(cx).session.as_ref(), self.pane) {
            store.update(cx, Store::close_dialog);
        }
    }

    fn ask_cached(&mut self, cx: &mut Context<Self>) {
        self.reads.start();
        store::run_partial(cx, read::cached_args(self.pane), |this, result, cx| {
            this.reads.settle(outcome_of(result), Instant::now());
            if this.reads.read_follows(shown(cx)) {
                this.read(cx);
            }
            cx.notify();
        });
        cx.notify();
    }

    fn read(&mut self, cx: &mut Context<Self>) {
        if !self.reads.can_read() {
            return;
        }
        self.reads.start();
        let args = read::read_args(self.pane);
        let who = self.reads.answer().and_then(|a| a.title.clone()).unwrap_or_else(|| "the agent".into());
        let busy = Busy::new(format!("context:{}", self.pane), format!("Reading {who}'s context…"));
        store::run_local_busy(
            cx,
            busy,
            move || cli::run_partial(&args),
            |this, result, cx| {
                this.reads.settle(outcome_of(result), Instant::now());
                cx.notify();
            },
        );
        cx.notify();
    }
}

impl Render for ContextDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let height = body_height(f32::from(window.viewport_size().height));
        let home = Store::get(cx).read(cx).session.as_ref().and_then(|s| s.home.clone()).unwrap_or_default();
        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                if e.keystroke.key == "enter" {
                    this.read(cx);
                }
            }))
            .child(frame("Context", body(&self.reads, &home, height), footer(&self.reads, cx)).w(px(560.)))
    }
}

fn body(reads: &Reads, home: &str, height: f32) -> Div {
    let quiet = reads.answer().is_some_and(|a| a.notice.is_none());
    div()
        .max_h(px(height))
        .flex()
        .flex_col()
        .gap(px(12.))
        .children(reads.failure().map(|why| ui::error_line(format!("Could not read the context: {why}"))))
        .child(numbers(reads, home))
        .when(quiet, |d| d.child(muted(NOTHING_SPENT).text_size(px(11.5)).text_color(colours::faint())))
}

fn numbers(reads: &Reads, home: &str) -> Stateful<Div> {
    let answer = reads.answer();
    div()
        .id("context-body")
        .min_h_0()
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap(px(12.))
        .when(reads.too_old(), |d| d.child(muted(TOO_OLD)))
        .when(reads.running() && answer.is_none(), |d| d.child(ui::busy("context-asking", "Reading…")))
        .children(answer.map(|a| heading(a, reads.waits_for_numbers())))
        .children(answer.and_then(|a| a.notice.clone()).map(muted))
        .children(answer.and_then(|a| a.breakdown.as_ref()).map(|b| filling(b, home)))
}

fn muted(text: impl Into<SharedString>) -> Div {
    div().text_size(px(12.5)).text_color(colours::muted()).child(text.into())
}

fn heading(answer: &ContextAnswer, waits: bool) -> Div {
    let compacts = answer.breakdown.as_ref().and_then(lists::compacts_line);
    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(div().text_size(px(14.)).child(lists::agent_line(answer)))
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(8.))
                .text_size(px(12.5))
                .text_color(colours::soft())
                .children(lists::fill_line(answer))
                .children(compacts.map(ui::quiet_chip)),
        )
        .when(waits, |d| d.child(ui::busy("context-reading", "Reading what fills it, 3 to 10 s…").pt(px(8.))))
}

fn filling(breakdown: &Breakdown, home: &str) -> Div {
    let slices = lists::slices(breakdown);
    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .children(lists::no_conversation(breakdown).map(muted))
        .child(bar(&slices))
        .child(div().flex().flex_col().gap(px(2.)).children(slices.iter().map(legend_row)))
        .children(lists::not_loaded(breakdown).map(muted))
        .child(div().flex().flex_col().children(lists::sections(breakdown, home).iter().map(section)))
}

fn ink(fill: Fill) -> Hsla {
    match fill {
        Fill::Used(at) => theme::hsla(theme::palette(PALETTE[at % PALETTE.len()])),
        Fill::Free => colours::border().into(),
        Fill::Buffer => glass::tint(colours::faint(), 0.6).into(),
    }
}

fn gives_way(mut part: Div) -> Div {
    part.style().flex_shrink = Some(GIVES_WAY);
    part
}

fn bar_slice(slice: &Slice) -> Div {
    let part = div().h_full().min_w(px(2.)).rounded(px(3.)).bg(ink(slice.fill)).w(relative(slice.fraction));
    if slice.fill == Fill::Free { gives_way(part) } else { part }
}

fn bar(slices: &[Slice]) -> Div {
    let parts = slices.iter().filter(|s| s.fraction > 0.).map(bar_slice);
    div().flex().w_full().h(px(BAR_HEIGHT)).gap(px(2.)).children(parts)
}

fn figure(text: impl Into<SharedString>, width: f32) -> Div {
    div().w(px(width)).flex_none().text_right().text_color(colours::muted()).child(text.into())
}

fn legend_row(slice: &Slice) -> Div {
    let used = matches!(slice.fill, Fill::Used(_));
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .text_size(px(12.5))
        .text_color(if used { colours::soft() } else { colours::muted() })
        .child(div().size(px(SWATCH)).flex_none().rounded(px(3.)).bg(ink(slice.fill)))
        .child(div().flex_1().min_w_0().child(ui::one_line(slice.label.clone())))
        .child(figure(slice.tokens.clone(), TOKENS_WIDTH))
        .child(figure(slice.share.clone(), SHARE_WIDTH))
}

fn section(section: &Section) -> Div {
    div().flex().flex_col().gap(px(2.)).child(ui::section(section.title)).children(section.rows.iter().map(row_line))
}

fn row_line(row: &Row) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .text_size(px(12.5))
        .text_color(if row.muted { colours::muted() } else { colours::soft() })
        .child(left_cell(row))
        .when(!row.middle.is_empty(), |d| {
            d.child(div().flex_none().text_size(px(11.5)).text_color(colours::muted()).child(row.middle.clone()))
        })
        .child(figure(row.right.clone(), TOKENS_WIDTH))
}

fn left_cell(row: &Row) -> Div {
    let cell = div().flex_1().min_w_0();
    if row.folder.is_empty() {
        return cell.child(ui::one_line(row.left.clone()));
    }
    cell.flex().child(gives_way(path_part(&row.folder)).text_color(colours::muted())).child(path_part(&row.left))
}

fn path_part(text: &str) -> Div {
    div().flex().min_w_0().child(ui::one_line(text.to_string()))
}

fn footer(reads: &Reads, cx: &mut Context<ContextDialog>) -> Div {
    div()
        .flex_1()
        .flex()
        .items_center()
        .gap(px(8.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(12.))
                .text_color(colours::muted())
                .children(reads.age_note(Instant::now()).map(ui::one_line)),
        )
        .child(ui::button("context-done", "Done").on_click(|_: &ClickEvent, _, cx| close(cx)))
        .children(refresh_button(reads, cx))
}

fn refresh_button(reads: &Reads, cx: &mut Context<ContextDialog>) -> Option<Stateful<Div>> {
    if reads.too_old() {
        return None;
    }
    let button = if reads.running() {
        ui::busy_button("context-refresh", "Reading…", Tone::Primary)
    } else {
        ui::icon_text_button("context-refresh", Icon::Refresh, "Refresh", Tone::Primary)
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.read(cx)))
    };
    Some(button.min_w(px(REFRESH_WIDTH)).justify_center())
}

#[cfg(test)]
mod tests;
