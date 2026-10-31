use gpui::{
    ClickEvent, Context, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _, Rgba, SharedString,
    StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _, px,
};

use super::ProcessesPanel;
use super::data::{Heat, Order, Sample, counts, cpu, heat, load, memory};
use crate::panels::poll::Polled;
use crate::theme::{FONT_FAMILY, colours};
use crate::ui::{self, Icon};

pub(super) const LIST_PAD: f32 = 16.;
pub(super) const CARD_PAD: f32 = 6.;
pub(super) const ROW_PAD: f32 = 6.;
pub(super) const LEAD: f32 = 44.;
pub(super) const ACTION: f32 = 60.;
const PID: f32 = 56.;
const CPU: f32 = 108.;
const MEMORY: f32 = 76.;
const COLUMN_GAP: f32 = 18.;
const METER: f32 = 36.;
const CARD_BORDER: f32 = 1.;
const INSET: f32 = LIST_PAD + CARD_BORDER + CARD_PAD + ROW_PAD;

pub(super) fn cell(width: f32) -> Div {
    div().w(px(width)).flex_none().flex().items_center().justify_end()
}

fn columns(pid: impl IntoElement, cpu: impl IntoElement, memory: impl IntoElement) -> Div {
    div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(COLUMN_GAP))
        .child(cell(PID).child(pid))
        .child(cell(CPU).child(cpu))
        .child(cell(MEMORY).child(memory))
}

pub(super) fn figures(pid: Option<u64>, cpu: impl IntoElement, memory: impl IntoElement) -> Div {
    let text = div().font_family(FONT_FAMILY).text_size(px(11.)).text_color(colours::faint());
    columns(text.children(pid.map(|p| p.to_string())), cpu, memory)
}

fn heat_colour(heat: Heat, strong: bool) -> Rgba {
    match heat {
        Heat::Hot => colours::waiting(),
        Heat::Busy if strong => colours::text(),
        Heat::Busy => colours::soft(),
        Heat::Idle => colours::faint(),
    }
}

pub(super) fn cpu_text(value: Option<f64>, strong: bool) -> Div {
    let heat = heat(value);
    div()
        .text_size(px(12.))
        .text_color(heat_colour(heat, strong))
        .when(heat == Heat::Hot, |d| d.font_weight(FontWeight::SEMIBOLD))
        .child(cpu(value))
}

pub(super) fn cpu_with_meter(value: Option<f64>) -> Div {
    let heat = heat(value);
    let colour = if heat == Heat::Hot { colours::waiting() } else { colours::accent() };
    let meter = (heat != Heat::Idle).then(|| ui::bar(load(value), colour, 4.));
    div()
        .w_full()
        .flex()
        .items_center()
        .gap(px(10.))
        .child(div().w(px(METER)).flex_none().children(meter))
        .child(div().flex_1().flex().justify_end().child(cpu_text(value, true)))
}

pub(super) fn memory_text(value: Option<u64>, strong: bool) -> Div {
    div().text_size(px(12.)).text_color(if strong { colours::text() } else { colours::soft() }).child(memory(value))
}

fn label(text: &'static str) -> Div {
    div().text_size(px(11.)).font_weight(FontWeight::MEDIUM).child(text.to_uppercase())
}

fn total(text: String) -> Div {
    div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).text_color(colours::text()).child(text)
}

impl ProcessesPanel {
    fn sorter(&self, order: Order, text: &'static str, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let active = self.order == order;
        let colour = if active { colours::accent() } else { colours::faint() };
        div()
            .id(SharedString::from(format!("sort-{text}")))
            .flex()
            .items_center()
            .gap(px(3.))
            .cursor_pointer()
            .text_color(colour)
            .child(label(text))
            .when(active, |d| d.child(ui::icon(Icon::ChevronDown, 10.).text_color(colour)))
            .when(!active, |d| d.hover(|s| s.text_color(colours::soft())))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_order(order, cx)))
    }

    pub(super) fn column_header(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .flex_none()
            .flex()
            .items_center()
            .h(px(24.))
            .px(px(INSET))
            .child(div().w(px(LEAD)).flex_none())
            .child(div().flex_1().flex().child(self.sorter(Order::Sidebar, "Pane", cx)))
            .child(columns(
                label("PID").text_color(colours::faint()),
                self.sorter(Order::Cpu, "CPU", cx),
                self.sorter(Order::Memory, "Memory", cx),
            ))
            .child(div().w(px(ACTION)).flex_none())
    }

    pub(super) fn footer(sample: &Sample) -> impl IntoElement + use<> {
        let note = format!(
            "CPU is the share of one core over {} ms, measured every {} s while this is open.",
            sample.interval,
            Self::EVERY.as_secs()
        );
        let (cpu_total, memory_total) = (cpu(Some(sample.total.cpu)), memory(Some(sample.total.memory)));
        div()
            .flex_none()
            .flex()
            .items_center()
            .h(px(38.))
            .px(px(INSET))
            .border_t_1()
            .border_color(colours::border())
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(div().flex_none().text_size(px(12.)).text_color(colours::muted()).child(counts(sample)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(11.))
                            .text_color(colours::faint())
                            .child(ui::one_line(note)),
                    ),
            )
            .child(figures(None, total(cpu_total), total(memory_total)))
            .child(div().w(px(ACTION)).flex_none())
    }
}
