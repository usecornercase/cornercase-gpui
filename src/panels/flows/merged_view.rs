use std::time::Instant;

use gpui::{
    ClickEvent, Context, Div, IntoElement, ParentElement as _, SharedString, StatefulInteractiveElement as _,
    Styled as _, div, px,
};

use super::FlowsPanel;
use super::actions::toast;
use super::merged::{LABEL, Switch, doing, how, key, parse_set, switch, switch_args, take};
use super::model::Run;
use crate::cli;
use crate::model::Session;
use crate::store::{self, Busy, Store, ToastKind};
use crate::ui;

pub(super) fn with_line(the_box: impl IntoElement, line: impl Into<SharedString>) -> Div {
    div().flex().flex_col().gap(px(4.)).child(the_box).child(ui::dim(line.into()))
}

impl FlowsPanel {
    fn set_close_when_merged(workspace: u64, flow: &str, on: bool, cx: &mut Context<Self>) {
        let busy = Busy::new(key(workspace), doing(flow, on));
        store::run_busy(cx, busy, switch_args(workspace, on), |this, result, cx| {
            match result.and_then(|out| parse_set(&out)) {
                Ok(set) => {
                    take(&mut this.runs, &set);
                    cx.notify();
                }
                Err(e) => toast(cli::reason(&e), ToastKind::Error, cx),
            }
        });
    }
}

pub(super) fn run_switch(run: &Run, cx: &Context<FlowsPanel>) -> Option<Div> {
    let here = Store::get(cx).read(cx).session.as_ref().and_then(Session::release);
    match switch(run, here) {
        Switch::Hidden => None,
        Switch::Rests(line) => Some(with_line(ui::resting_checkbox(false, LABEL), line)),
        Switch::Offered(on) => Some(offered_switch(run, on, cx)),
    }
}

fn offered_switch(run: &Run, on: bool, cx: &Context<FlowsPanel>) -> Div {
    let store = Store::get(cx).read(cx);
    let key = key(run.workspace);
    if let Some(doing) = store.jobs.doing(&key).filter(|_| store.jobs.shown(&key, Instant::now())) {
        return with_line(ui::busy("run-merged-busy", doing.to_string()), how(None));
    }
    let (workspace, flow) = (run.workspace, run.flow.name.clone());
    let the_box = ui::checkbox("run-merged", on, LABEL).on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
        FlowsPanel::set_close_when_merged(workspace, &flow, !on, cx);
    }));
    with_line(the_box, how(None))
}
