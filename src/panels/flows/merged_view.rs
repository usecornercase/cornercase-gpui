use std::time::Instant;

use gpui::{
    ClickEvent, Context, Div, IntoElement, ParentElement as _, SharedString, StatefulInteractiveElement as _,
    Styled as _, div, px,
};

use super::FlowsPanel;
use super::actions::toast;
use super::merged::{
    Choice, FREE_HOW, FREE_LABEL, LABEL, Press, Switch, doing, free_key, free_switch, how, key, parse_set, switch,
    switch_args, take,
};
use super::model::Run;
use crate::cli;
use crate::model::Session;
use crate::store::{self, Busy, Store, ToastKind};
use crate::ui;

pub(super) fn with_line(the_box: impl IntoElement, line: impl Into<SharedString>) -> Div {
    div().flex().flex_col().gap(px(4.)).child(the_box).child(ui::dim(line.into()))
}

impl FlowsPanel {
    fn set_choice(workspace: u64, flow: &str, press: Press, choice: Choice, cx: &mut Context<Self>) {
        let jobs = &Store::get(cx).read(cx).jobs;
        if jobs.doing(&key(workspace)).is_some() || jobs.doing(&free_key(workspace)).is_some() {
            return;
        }
        let to = choice.pressed(press);
        let job = if press == Press::Close { key(workspace) } else { free_key(workspace) };
        let busy = Busy::new(job, doing(flow, press, to));
        store::run_busy(cx, busy, switch_args(workspace, to), |this, result, cx| {
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
        Switch::Offered(_) => Some(offered_pair(run, Choice::of(run, here), free_switch(run, here), cx)),
    }
}

fn offered_pair(run: &Run, choice: Choice, free: Switch, cx: &Context<FlowsPanel>) -> Div {
    let first = Row { run, press: Press::Close, choice, line: how(None, choice.free) };
    let second = match free {
        Switch::Hidden => None,
        Switch::Rests(line) => Some(free_resting(line)),
        Switch::Offered(_) => Some(nested(row(&Row { press: Press::Free, line: FREE_HOW.into(), ..first }, cx))),
    };
    pair(row(&first, cx), second)
}

pub(super) fn pair(first: Div, second: Option<Div>) -> Div {
    div().flex().flex_col().gap(px(8.)).child(first).children(second)
}

pub(super) fn free_resting(line: String) -> Div {
    nested(with_line(ui::resting_checkbox(false, FREE_LABEL), line))
}

pub(super) fn free_offered(the_box: impl IntoElement) -> Div {
    nested(with_line(the_box, FREE_HOW))
}

fn nested(body: Div) -> Div {
    body.ml(px(ui::TICK_BOX + 8.))
}

#[derive(Clone)]
struct Row<'a> {
    run: &'a Run,
    press: Press,
    choice: Choice,
    line: String,
}

fn row(row: &Row, cx: &Context<FlowsPanel>) -> Div {
    let store = Store::get(cx).read(cx);
    let (own, other) = match row.press {
        Press::Close => (key(row.run.workspace), free_key(row.run.workspace)),
        Press::Free => (free_key(row.run.workspace), key(row.run.workspace)),
    };
    let shown = |key: &str| store.jobs.doing(key).filter(|_| store.jobs.shown(key, Instant::now()));
    let (id, busy_id, label, on) = match row.press {
        Press::Close => ("run-merged", "run-merged-busy", LABEL, row.choice.close),
        Press::Free => ("run-free", "run-free-busy", FREE_LABEL, row.choice.free),
    };
    if let Some(doing) = shown(&own) {
        return with_line(ui::busy(busy_id, doing.to_string()), row.line.clone());
    }
    if shown(&other).is_some() {
        return with_line(ui::resting_checkbox(on, label), row.line.clone());
    }
    let (workspace, flow, press, choice) = (row.run.workspace, row.run.flow.name.clone(), row.press, row.choice);
    let the_box = ui::checkbox(id, on, label).on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
        FlowsPanel::set_choice(workspace, &flow, press, choice, cx);
    }));
    with_line(the_box, row.line.clone())
}
