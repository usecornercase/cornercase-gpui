use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    Animation, AnimationExt as _, AnyElement, Context, Div, FontWeight, IntoElement, ParentElement as _, Rgba,
    SharedString, Styled as _, div, ease_in_out, px, rgb, rgba,
};

use super::FlowsPanel;
use super::cards::{ground, lead_card, role_card};
use super::graph::{self, Curve, Graph, Label, Rect, Spot, Wire};
use super::model::{Edge, FlowSpec, Member, Progress, Run, Step};
use super::wires::{self, Ink, Line};
use crate::theme::{colours, radius};
use crate::ui::{self, Icon, glass, glass::tint, motion};

const ROLE_COLOURS: [u32; 6] = [0x007a_a2f7, 0x00bb_9af7, 0x007d_cfff, 0x009e_ce6a, 0x0073_daca, 0x00e0_af68];
const ENTER: Duration = Duration::from_millis(340);
const STAGGER: Duration = Duration::from_millis(80);
const TRAVEL: Duration = Duration::from_secs(1);
const RISE: f32 = 12.;
const MARK: f32 = 24.;
const BOARD_PAD: f32 = 18.;

pub(super) fn role_colour(flow: &FlowSpec, role: &str) -> Rgba {
    match flow.colour_index(role) {
        Some(i) => rgb(ROLE_COLOURS[i % ROLE_COLOURS.len()]),
        None => colours::soft(),
    }
}

#[derive(Default)]
pub(super) struct Played(RefCell<(String, HashMap<String, Instant>)>);

impl Played {
    fn since(&self, key: &str, animation: String, now: Instant) -> Duration {
        let mut played = self.0.borrow_mut();
        if played.0 != key {
            *played = (key.to_string(), HashMap::new());
        }
        now.saturating_duration_since(*played.1.entry(animation).or_insert(now))
    }
}

#[derive(Clone, Copy)]
pub(super) struct Plan<'a> {
    pub flow: &'a FlowSpec,
    pub run: Option<&'a Run>,
    pub focus: Option<&'a str>,
    pub key: &'a str,
}

impl<'a> Plan<'a> {
    pub(super) fn member(self, role: &str) -> Option<&'a Member> {
        self.run?.member(role)
    }

    pub(super) fn holder(self, role: &str) -> bool {
        self.run.and_then(|r| r.holder.as_deref()) == Some(role)
    }

    fn last(self) -> Option<&'a Step> {
        self.run?.last.as_ref()
    }

    fn progress(self, stage: usize) -> Option<Progress> {
        self.run.map(|r| r.progress(stage))
    }

    fn ended(self) -> bool {
        self.run.is_some_and(|r| r.ended)
    }

    fn colour(self, role: Option<&str>) -> Rgba {
        role.map_or(colours::soft(), |r| role_colour(self.flow, r))
    }

    fn lit(self) -> Rgba {
        self.colour(self.last().and_then(|s| s.from.as_deref()))
    }
}

impl FlowsPanel {
    pub(super) fn diagram(&self, plan: Plan, cx: &Context<Self>) -> Div {
        let room = self.board.get() - 2. * BOARD_PAD;
        let graph = graph::fit(plan.flow, plan.run.and_then(|r| r.outside), room);
        let field = div()
            .relative()
            .flex_none()
            .mx_auto()
            .w(px(graph.width))
            .h(px(graph.height))
            .children(layers(plan, &graph, &self.played, cx))
            .children(travel(plan, &graph, &self.played));
        board(self.board.clone()).child(field).children(plan.run.and_then(|r| r.outside).and_then(outside_note))
    }
}

fn layers(plan: Plan, graph: &Graph, played: &Played, cx: &Context<FlowsPanel>) -> Vec<AnyElement> {
    let stages = graph.markers.len();
    let lead = rise(lead_layer(plan, graph, cx), plan.key, 0, played);
    let middle = (0..stages).map(|stage| rise(stage_layer(plan, graph, stage, cx), plan.key, stage + 1, played));
    let end = rise(end_layer(plan, graph), plan.key, stages + 1, played);
    std::iter::once(lead).chain(middle).chain(std::iter::once(end)).collect()
}

fn lead_layer(plan: Plan, graph: &Graph, cx: &Context<FlowsPanel>) -> Div {
    layer().child(wires::lines(vec![divider(graph)])).child(caption(graph)).child(lead_card(plan, graph.lead, cx))
}

fn stage_layer(plan: Plan, graph: &Graph, stage: usize, cx: &Context<FlowsPanel>) -> Div {
    let cards = graph.cards.iter().enumerate().filter(|(_, c)| c.stage == stage);
    let labels = graph.labels.iter().filter(|l| l.stage == stage);
    layer()
        .child(wires::lines(stage_lines(plan, graph, stage)))
        .child(marker(plan, graph.markers.get(stage).copied().unwrap_or_default(), stage))
        .children(cards.map(|card| role_card(plan, card, cx)))
        .children(labels.filter_map(|l| loop_label(plan, *l, graph.short)))
}

fn board(room: Rc<Cell<f32>>) -> Div {
    div()
        .relative()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(14.))
        .py(px(26.))
        .px(px(BOARD_PAD))
        .rounded(px(radius::CARD))
        .bg(rgba(0x00_00_00_33))
        .border_1()
        .border_color(colours::border())
        .overflow_hidden()
        .child(wires::backdrop(rgba(0xff_ff_ff_10), room))
}

fn layer() -> Div {
    div().absolute().top_0().left_0().size_full()
}

fn rise(layer: Div, key: &str, index: usize, played: &Played) -> AnyElement {
    let delay = STAGGER * u32::try_from(index.min(12)).unwrap_or_default();
    let total = ENTER + delay;
    if played.since(key, format!("in-{index}"), Instant::now()) >= total {
        return layer.into_any_element();
    }
    let start = delay.as_secs_f32() / total.as_secs_f32();
    let id = (SharedString::from(format!("flow-in-{key}")), index);
    layer
        .with_animation(id, Animation::new(total), move |layer, t| {
            let shown = motion::ease_out(((t - start) / (1. - start)).clamp(0., 1.));
            layer.opacity(shown).top(px((1. - shown) * RISE))
        })
        .into_any_element()
}

fn travel(plan: Plan, graph: &Graph, played: &Played) -> Option<AnyElement> {
    let step = plan.last()?;
    if played.since(plan.key, format!("travel-{}-{}", step.time, step.to), Instant::now()) >= TRAVEL {
        return None;
    }
    let wire = graph.wires.iter().find(|w| graph.carries(w, step))?.clone();
    let colour = plan.lit();
    let id = SharedString::from(format!("flow-travel-{}-{}-{}", plan.key, step.time, step.to));
    let element = layer().with_animation(id, Animation::new(TRAVEL).with_easing(ease_in_out), move |layer, t| {
        let shown = ((1. - t) / 0.2).clamp(0., 1.);
        layer.child(wires::spark(wire.at(t), colour, shown))
    });
    Some(element.into_any_element())
}

fn belongs(wire: &Wire, stage: usize, last: bool) -> bool {
    match wire.edge {
        Some(Edge::Forward { stage: from } | Edge::Loop { stage: from }) => from == stage,
        Some(Edge::Outside { to, .. }) => to == stage,
        None => last,
    }
}

fn stage_lines(plan: Plan, graph: &Graph, stage: usize) -> Vec<Line> {
    let last = stage + 1 == graph.markers.len();
    let (lit, dull): (Vec<&Wire>, Vec<&Wire>) = graph
        .wires
        .iter()
        .filter(|w| belongs(w, stage, last))
        .partition(|w| plan.last().is_some_and(|step| graph.carries(w, step)));
    let wires = dull.into_iter().map(|w| (w, false)).chain(lit.into_iter().map(|w| (w, true)));
    std::iter::once(rail(plan, graph, stage))
        .chain(wires.map(|(w, lit)| Line {
            curves: w.curves.clone(),
            tip: Some((w.tip, w.way)),
            ink: ink(plan, w, lit),
        }))
        .collect()
}

fn ink(plan: Plan, wire: &Wire, lit: bool) -> Ink {
    let start = if lit { plan.lit() } else { plan.colour(wire.from.as_deref()) };
    let end = wire.to.as_deref().map_or_else(|| end_colour(plan), |r| role_colour(plan.flow, r));
    let over = match wire.edge {
        Some(Edge::Loop { stage }) => plan.run.is_some_and(|r| r.over_limit(stage)),
        Some(Edge::Outside { .. }) => true,
        _ => false,
    };
    if over {
        let dashed = matches!(wire.edge, Some(Edge::Outside { .. }));
        return Ink { dashed, ..Ink::plain(colours::waiting(), colours::waiting(), 1.6) };
    }
    if lit || (wire.edge.is_none() && plan.ended()) {
        return Ink { glow: 0.22, ..Ink::plain(start, end, 2.2) };
    }
    let alpha = dull_alpha(plan, wire);
    Ink::plain(tint(start, alpha), tint(end, alpha), 1.5)
}

fn dull_alpha(plan: Plan, wire: &Wire) -> f32 {
    let looping = matches!(wire.edge, Some(Edge::Loop { .. }));
    let Some(run) = plan.run else { return if looping { 0.42 } else { 0.55 } };
    match wire.edge {
        Some(Edge::Forward { stage }) if run.progress(stage + 1) != Progress::Ahead => 0.55,
        Some(Edge::Loop { stage }) if run.rounds.get(stage).is_some_and(|n| *n > 0) => 0.45,
        _ => 0.16,
    }
}

fn end_colour(plan: Plan) -> Rgba {
    if plan.ended() { colours::accent() } else { colours::soft() }
}

fn rail(plan: Plan, graph: &Graph, stage: usize) -> Line {
    let from = graph.markers.get(stage).copied().unwrap_or_default();
    let to = graph.markers.get(stage + 1).copied().unwrap_or_else(|| graph.end_marker());
    let reached = match plan.run {
        Some(run) if stage + 1 < graph.markers.len() => run.progress(stage + 1) != Progress::Ahead,
        Some(run) => run.ended,
        None => false,
    };
    let ink = if reached {
        Ink { glow: 0.18, ..Ink::plain(colours::brand_light(), colours::brand(), 2.) }
    } else {
        Ink::plain(rgba(0xff_ff_ff_1c), rgba(0xff_ff_ff_1c), 2.)
    };
    let gap = MARK / 2. + 4.;
    let curve = Curve::line(Spot { x: from.x, y: from.y + gap }, Spot { x: to.x, y: to.y - gap });
    Line { curves: vec![curve], tip: None, ink }
}

fn divider(graph: &Graph) -> Line {
    let (left, right) = graph.column;
    let y = graph.divider;
    let faint = rgba(0xff_ff_ff_24);
    let curve = Curve::line(Spot { x: left, y }, Spot { x: right, y });
    Line { curves: vec![curve], tip: None, ink: Ink { dashed: true, ..Ink::plain(faint, faint, 1.) } }
}

fn caption(graph: &Graph) -> Div {
    let (left, right) = graph.column;
    div()
        .absolute()
        .left(px(left))
        .w(px(right - left))
        .top(px(graph.divider - 11.))
        .h(px(22.))
        .flex()
        .justify_center()
        .child(
            div()
                .h_full()
                .flex()
                .items_center()
                .px(px(10.))
                .rounded_full()
                .bg(ground())
                .border_1()
                .border_color(colours::border())
                .text_size(px(11.))
                .text_color(colours::muted())
                .child("moves the work between the stages"),
        )
}

fn marker(plan: Plan, at: Spot, stage: usize) -> Div {
    let base = div()
        .absolute()
        .left(px(at.x - MARK / 2.))
        .top(px(at.y - MARK / 2.))
        .size(px(MARK))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .child((stage + 1).to_string());
    match plan.progress(stage) {
        Some(Progress::Active) => base
            .bg(glass::cobalt())
            .border_color(tint(colours::brand_light(), 0.9))
            .text_color(colours::on_brand())
            .shadow(glass::glow(colours::brand(), 0.6, 14.)),
        Some(Progress::Passed) => base
            .bg(tint(colours::brand(), 0.3))
            .border_color(tint(colours::brand_light(), 0.6))
            .text_color(colours::text()),
        Some(Progress::Ahead) => base.bg(ground()).border_color(colours::border()).text_color(colours::faint()),
        None => base.bg(ground()).border_color(colours::edge()).text_color(colours::muted()),
    }
}

fn loop_label(plan: Plan, label: Label, short: bool) -> Option<Div> {
    let back = plan.flow.stages.get(label.stage)?.back.as_ref()?;
    let text = back.label(plan.run.and_then(|r| r.rounds.get(label.stage).copied()), short);
    let colour = loop_colour(plan, label.stage);
    Some(
        div()
            .absolute()
            .left(px(label.at.x))
            .top(px(label.at.y - 12.))
            .h(px(24.))
            .flex()
            .items_center()
            .gap(px(6.))
            .px(px(10.))
            .rounded_full()
            .bg(ground())
            .border_1()
            .border_color(tint(colour, 0.4))
            .whitespace_nowrap()
            .text_size(px(11.))
            .text_color(colour)
            .child(ui::icon(Icon::Repeat, 12.).text_color(colour))
            .child(text),
    )
}

fn loop_colour(plan: Plan, stage: usize) -> Rgba {
    match plan.run {
        Some(run) if run.over_limit(stage) => colours::waiting(),
        Some(_) if plan.last().and_then(|s| s.edge) == Some(Edge::Loop { stage }) => plan.lit(),
        _ => colours::muted(),
    }
}

fn end_layer(plan: Plan, graph: &Graph) -> Div {
    let colour = if plan.ended() { colours::accent() } else { rgba(0xff_ff_ff_38) };
    layer().child(wires::diamond(graph.end_marker(), 6., colour)).child(end_pill(plan, graph.end))
}

fn end_pill(plan: Plan, rect: Rect) -> Div {
    let glyph = if plan.flow.ends_in_commits() { Icon::Commit } else { Icon::PullRequest };
    let pill = div()
        .absolute()
        .left(px(rect.x))
        .top(px(rect.y))
        .w(px(rect.w))
        .h(px(rect.h))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .gap(px(8.))
        .border_1()
        .text_size(px(12.));
    if plan.ended() {
        return pill
            .bg(glass::cobalt())
            .border_color(tint(colours::brand_light(), 0.8))
            .shadow(glass::glow(colours::brand(), 0.5, 18.))
            .text_color(colours::on_brand())
            .font_weight(FontWeight::MEDIUM)
            .child(ui::icon(glyph, 14.).text_color(colours::on_brand()))
            .child(format!("{} · reached", plan.flow.end_label()));
    }
    pill.bg(ground())
        .border_color(colours::edge())
        .text_color(colours::soft())
        .child(ui::icon(glyph, 14.).text_color(colours::muted()))
        .child(plan.flow.end_label())
}

fn outside_note(edge: Edge) -> Option<Div> {
    let Edge::Outside { from, to } = edge else { return None };
    let from = from.map_or("the start".to_string(), |f| format!("stage {}", f + 1));
    Some(
        div()
            .mx_auto()
            .px(px(10.))
            .py(px(3.))
            .rounded_full()
            .border_1()
            .border_dashed()
            .border_color(colours::waiting())
            .text_size(px(11.))
            .text_color(colours::waiting())
            .child(format!("outside the plan: {from} → stage {}", to + 1)),
    )
}

#[cfg(test)]
mod tests;
