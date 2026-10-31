use gpui::{
    AnyElement, ClickEvent, Context, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, Rgba,
    SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

use super::ProcessesPanel;
use super::columns::{ACTION, CARD_PAD, LEAD, ROW_PAD, cell, cpu_text, cpu_with_meter, figures, memory_text};
use super::data::{Found, Own, PaneUse, Process, Sample, folded, found, heading, ordered, place, under};
use crate::dialogs::Question;
use crate::panels::poll::{Stop, confirm_stop};
use crate::panels::queue::open_pane;
use crate::store::{Busy, Store};
use crate::theme::{FONT_FAMILY, colours, radius};
use crate::ui::{self, Icon};

const INDENT: f32 = 16.;
const HEAD: f32 = 36.;
const ROW: f32 = 26.;
const OWN_NOTE: &str = "The server and what it runs outside the panes: plugin services, popups, its own git jobs.";

fn card() -> Div {
    div()
        .flex()
        .flex_col()
        .px(px(CARD_PAD))
        .py(px(4.))
        .rounded(px(radius::CARD))
        .bg(colours::terminal())
        .border_1()
        .border_color(colours::border())
}

fn hovered_row(group: &SharedString, height: f32) -> Stateful<Div> {
    div()
        .id(group.clone())
        .group(group.clone())
        .flex()
        .items_center()
        .h(px(height))
        .px(px(ROW_PAD))
        .rounded(px(radius::CONTROL))
        .hover(|s| s.bg(colours::hover()))
}

fn revealed(group: SharedString, control: impl IntoElement) -> Div {
    div().flex().opacity(0.).group_hover(group, |s| s.opacity(1.)).child(control)
}

fn lead(toggle: Option<bool>, glyph: Icon, colour: Rgba) -> Div {
    let chevron = toggle.map(|open| {
        ui::icon(if open { Icon::ChevronDown } else { Icon::ChevronRight }, 12.).text_color(colours::faint())
    });
    div()
        .w(px(LEAD))
        .flex_none()
        .flex()
        .items_center()
        .gap(px(8.))
        .child(div().w(px(14.)).flex_none().flex().justify_center().children(chevron))
        .child(ui::icon(glyph, 14.).text_color(colour))
}

fn title(name: String, place: String) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .items_center()
        .gap(px(10.))
        .child(div().flex_none().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).child(name))
        .child(div().flex_1().min_w_0().text_size(px(12.)).text_color(colours::muted()).child(ui::one_line(place)))
}

fn stopping(pid: u64) -> String {
    format!("stop-process:{pid}")
}

fn ask_stop(process: &Process, window: &mut Window, cx: &mut Context<ProcessesPanel>) {
    let (program, _) = process.command();
    let question = Question::new(
        format!("Stop {program} ({})?", process.pid),
        "It and everything it started get SIGTERM, then SIGKILL after 3 s if they are still running. \
         The pane's shell stays.",
        "Stop",
        Vec::new(),
    );
    let busy = Busy::new(stopping(process.pid), format!("Stopping {program}…"));
    confirm_stop(question, busy, process.stop_command(), window, cx);
}

fn stop_slot(process: &Process, stop: Stop<ProcessesPanel>, group: SharedString) -> Div {
    let slot = cell(ACTION);
    match stop {
        Stop::Never => slot,
        Stop::Stopping => {
            slot.child(ui::spinner(SharedString::from(format!("stopping-{}", process.pid)), 12., colours::muted()))
        }
        Stop::Ask(panel) => {
            let stopped = process.clone();
            let button = ui::danger_button(SharedString::from(format!("stop-{}", process.pid)), "Stop")
                .px(px(10.))
                .py(px(2.))
                .on_click(move |_: &ClickEvent, window, cx| panel.update(cx, |_, cx| ask_stop(&stopped, window, cx)));
            slot.child(revealed(group, button))
        }
    }
}

fn guides(depth: usize) -> impl Iterator<Item = Div> {
    (0..depth).map(|_| {
        div().w(px(INDENT)).h_full().flex_none().pl(px(3.)).child(div().w(px(1.)).h_full().bg(colours::border()))
    })
}

fn command(depth: usize, process: &Process) -> Div {
    let (program, arguments) = process.command();
    div()
        .flex_1()
        .min_w_0()
        .h_full()
        .flex()
        .items_center()
        .font_family(FONT_FAMILY)
        .text_size(px(12.))
        .children(guides(depth))
        .child(div().flex_none().text_color(colours::soft()).child(program))
        .when(!arguments.is_empty(), |d| {
            d.child(div().flex_1().min_w_0().pl(px(7.)).text_color(colours::muted()).child(ui::one_line(arguments)))
        })
}

fn process_row(depth: usize, process: &Process, stop: Stop<ProcessesPanel>) -> Stateful<Div> {
    let stopping = matches!(stop, Stop::Stopping);
    let group = SharedString::from(format!("process-{}", process.pid));
    hovered_row(&group, ROW)
        .child(div().w(px(LEAD)).flex_none())
        .child(command(depth, process))
        .child(figures(Some(process.pid), cpu_text(process.cpu, false), memory_text(process.memory, false)))
        .child(stop_slot(process, stop, group))
        .when(stopping, |d| d.opacity(0.5))
}

fn more_row(pane: u64, hidden: usize, open: bool, cx: &mut Context<ProcessesPanel>) -> impl IntoElement + use<> {
    let label = if open { "Show fewer".to_string() } else { format!("Show {hidden} more") };
    div().flex().items_center().h(px(24.)).px(px(ROW_PAD)).child(div().w(px(LEAD)).flex_none()).child(
        div()
            .id(("more", pane))
            .cursor_pointer()
            .text_size(px(12.))
            .text_color(colours::faint())
            .hover(|s| s.text_color(colours::text()))
            .child(label)
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_open(pane, cx))),
    )
}

fn pane_glyph(pane: &PaneUse) -> (Icon, Rgba) {
    if pane.agent.is_some() { (Icon::Agent, colours::accent()) } else { (Icon::Terminal, colours::muted()) }
}

fn pane_head(pane: &PaneUse, place: String, toggle: Option<bool>, cx: &mut Context<ProcessesPanel>) -> Stateful<Div> {
    let id = pane.pane;
    let group = SharedString::from(format!("pane-{id}"));
    let (glyph, colour) = pane_glyph(pane);
    let named = div().id(("pane-name", id)).flex_1().min_w_0().flex().items_center();
    let named = match toggle {
        Some(_) => {
            named.cursor_pointer().on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_open(id, cx)))
        }
        None => named,
    };
    let go = ui::button(SharedString::from(format!("go-{id}")), "Go")
        .px(px(10.))
        .py(px(2.))
        .on_click(move |_: &ClickEvent, _, cx| open_pane(id, cx));
    hovered_row(&group, HEAD)
        .child(named.child(lead(toggle, glyph, colour)).child(title(heading(pane), place)))
        .child(figures(Some(pane.pid), cpu_with_meter(pane.cpu), memory_text(pane.memory, true)))
        .child(cell(ACTION).child(revealed(group, go)))
}

fn own_card(own: &Own) -> AnyElement {
    let rows: Vec<Stateful<Div>> =
        under(&own.processes, own.pid).into_iter().map(|(depth, p)| process_row(depth, p, Stop::Never)).collect();
    let head = div()
        .flex()
        .items_center()
        .h(px(HEAD))
        .px(px(ROW_PAD))
        .child(lead(None, Icon::Server, colours::muted()))
        .child(title("cornercase".into(), OWN_NOTE.into()))
        .child(figures(Some(own.pid).filter(|pid| *pid != 0), cpu_with_meter(own.cpu), memory_text(own.memory, true)))
        .child(cell(ACTION));
    card().child(head).children(rows).into_any_element()
}

impl ProcessesPanel {
    fn pane_card(&self, pane: &PaneUse, place: String, found_by: &str, cx: &mut Context<Self>) -> AnyElement {
        let open = self.open.contains(&pane.pane);
        let fold = folded(under(&pane.processes, pane.pid), open, found_by);
        let toggle = (fold.hidden > 0).then_some(open);
        let (panel, store) = (cx.entity(), Store::get(cx));
        let stop = |p: &Process| Stop::of(true, self.watch.stopping(&stopping(p.pid), store.read(cx)), &panel);
        let rows: Vec<Stateful<Div>> = fold.rows.iter().map(|(depth, p)| process_row(*depth, p, stop(p))).collect();
        card()
            .child(pane_head(pane, place, toggle, cx))
            .children(rows)
            .children(toggle.map(|open| more_row(pane.pane, fold.hidden, open, cx)))
            .into_any_element()
    }

    pub(super) fn cards(&self, sample: &Sample, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let session = Store::get(cx).read(cx).session.clone();
        let filter = self.filter.read(cx).text();
        let mut list = Vec::new();
        for pane in ordered(&sample.panes, self.order, self.held()) {
            let place = place(pane, session.as_ref());
            match found(pane, &place, &filter) {
                Found::Not => {}
                Found::Pane => list.push(self.pane_card(pane, place, "", cx)),
                Found::Inside => list.push(self.pane_card(pane, place, &filter, cx)),
            }
        }
        if filter.trim().is_empty() {
            list.extend(sample.cornercase.as_ref().map(own_card));
        }
        list
    }
}
