use gpui::{
    AnyElement, ClickEvent, Context, Div, IntoElement, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, div, px,
};

use super::QueuePanel;
use super::data::{Entry, notice, place_label};
use super::rows::{RUN_INDENT, Shown, Snapshot, base_row, notice_line, run_actions, state_icon, state_text};
use super::time::{local, when_label};
use crate::theme::colours;
use crate::ui;

const RUN_HEIGHT: f32 = 38.;
const RUN_KEEP: f32 = 244.;
const NUDGE: f32 = 4.;

fn run_line(run: &Entry, shown: &Shown, snapshot: &Snapshot) -> Div {
    let when = local(run.at).map_or_else(|| run.at.to_string(), |at| when_label(at, snapshot.today));
    let place = place_label(&run.place, snapshot.session.as_ref());
    div()
        .flex()
        .gap(px(6.))
        .text_size(px(11.5))
        .child(div().flex_none().text_color(colours::soft()).child(when))
        .child(state_text(run, shown))
        .child(div().flex_1().min_w(px(0.)).text_color(colours::faint()).child(ui::one_line(format!("· {place}"))))
}

fn run_text(run: &Entry, shown: &Shown, snapshot: &Snapshot) -> Div {
    div()
        .flex_1()
        .min_w(px(0.))
        .flex()
        .flex_col()
        .gap(px(2.))
        .pt(px(NUDGE))
        .child(run_line(run, shown, snapshot))
        .children(notice(run).map(|text| notice_line(text, shown.state)))
}

impl QueuePanel {
    pub(super) fn run_row(&self, run: &Entry, snapshot: &Snapshot, cx: &mut Context<Self>) -> AnyElement {
        let shown = self.shown(run, snapshot, cx);
        let group = SharedString::from(format!("queue-row-{}", run.id));
        let (click, clicked) = (shown.click, run.clone());
        let row = base_row(run.id, false, click, group.clone())
            .flex_1()
            .min_w(px(RUN_KEEP))
            .min_h(px(RUN_HEIGHT))
            .gap(px(8.))
            .px(px(12.))
            .py(px(4.))
            .child(state_icon(run.id, shown.state).mt(px(NUDGE)))
            .child(run_text(run, &shown, snapshot))
            .child(run_actions(run, &shown, group, cx))
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.clicked(click, &clicked, window, cx)));
        div().flex().child(div().w(px(RUN_INDENT))).child(row).into_any_element()
    }
}
