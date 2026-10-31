use super::model::{BackSpec, Edge, FlowSpec, Step};

pub(crate) const CARD_W: f32 = 176.;
const MIN_CARD: f32 = 132.;
pub(crate) const CARD_H: f32 = 58.;
pub(crate) const LEAD_W: f32 = 252.;
pub(crate) const LEAD_H: f32 = 56.;
pub(crate) const END_W: f32 = 228.;
pub(crate) const END_H: f32 = 38.;
pub(crate) const RAIL: f32 = 14.;
pub(crate) const HEAD: f32 = 7.;
const GAP_X: f32 = 16.;
const STAGE_GAP: f32 = 56.;
const COLUMN: f32 = 56.;
const DIVIDER: f32 = 36.;
const FIRST: f32 = 36.;
const END_GAP: f32 = 48.;
const LANE: f32 = 30.;
const BEND: f32 = 14.;
const SPREAD: f32 = 12.;
const NEAR: f32 = 3.;
const LABEL_GAP: f32 = 14.;
const CHAR_W: f32 = 6.2;
const LABEL_PAD: f32 = 44.;
const MARGIN: f32 = 8.;
const KAPPA: f32 = 0.552;
const PULL: f32 = 0.55;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Spot {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    fn centre_x(self) -> f32 {
        self.x + self.w / 2.
    }

    pub(crate) fn mid_y(self) -> f32 {
        self.y + self.h / 2.
    }

    fn right(self) -> f32 {
        self.x + self.w
    }

    fn bottom(self) -> f32 {
        self.y + self.h
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Curve {
    pub from: Spot,
    pub a: Spot,
    pub b: Spot,
    pub to: Spot,
}

impl Curve {
    pub(crate) fn line(from: Spot, to: Spot) -> Self {
        let third = |t: f32| Spot { x: from.x + (to.x - from.x) * t, y: from.y + (to.y - from.y) * t };
        Self { from, a: third(1. / 3.), b: third(2. / 3.), to }
    }

    fn corner(from: Spot, to: Spot, across_first: bool) -> Self {
        let (a, b) = if across_first {
            (
                Spot { x: from.x + (to.x - from.x) * KAPPA, y: from.y },
                Spot { x: to.x, y: to.y + (from.y - to.y) * KAPPA },
            )
        } else {
            (
                Spot { x: from.x, y: from.y + (to.y - from.y) * KAPPA },
                Spot { x: to.x + (from.x - to.x) * KAPPA, y: to.y },
            )
        };
        Self { from, a, b, to }
    }

    fn drop(from: Spot, to: Spot) -> Self {
        let pull = (to.y - from.y) * PULL;
        Self { from, a: Spot { x: from.x, y: from.y + pull }, b: Spot { x: to.x, y: to.y - pull }, to }
    }

    pub(crate) fn at(&self, t: f32) -> Spot {
        let u = 1. - t;
        let (w0, w1, w2, w3) = (u * u * u, 3. * u * u * t, 3. * u * t * t, t * t * t);
        Spot {
            x: w0 * self.from.x + w1 * self.a.x + w2 * self.b.x + w3 * self.to.x,
            y: w0 * self.from.y + w1 * self.a.y + w2 * self.b.y + w3 * self.to.y,
        }
    }

    fn length(&self) -> f32 {
        let chord = distance(self.from, self.to);
        let hull = distance(self.from, self.a) + distance(self.a, self.b) + distance(self.b, self.to);
        f32::midpoint(chord, hull)
    }
}

fn distance(p: Spot, q: Spot) -> f32 {
    (q.x - p.x).hypot(q.y - p.y)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Way {
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Wire {
    pub edge: Option<Edge>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub curves: Vec<Curve>,
    pub tip: Spot,
    pub way: Way,
}

impl Wire {
    pub(crate) fn at(&self, t: f32) -> Spot {
        let total: f32 = self.curves.iter().map(Curve::length).sum();
        let mut left = t.clamp(0., 1.) * total;
        for curve in &self.curves {
            let length = curve.length();
            if left <= length && length > 0. {
                return curve.at(left / length);
            }
            left -= length;
        }
        self.tip
    }

    pub(crate) fn carries(&self, step: &Step, sources: &[&str]) -> bool {
        if step.edge.is_none() || self.edge != step.edge {
            return false;
        }
        if !matches!(step.edge, Some(Edge::Forward { .. })) {
            return true;
        }
        let into = self.to.as_deref() == Some(step.to.as_str());
        let left = step.from.as_deref().filter(|from| sources.contains(from));
        into && left.is_none_or(|from| self.from.as_deref() == Some(from))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Card {
    pub role: String,
    pub stage: usize,
    pub rect: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Label {
    pub stage: usize,
    pub at: Spot,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Graph {
    pub width: f32,
    pub height: f32,
    pub lead: Rect,
    pub divider: f32,
    pub column: (f32, f32),
    pub cards: Vec<Card>,
    pub markers: Vec<Spot>,
    pub wires: Vec<Wire>,
    pub labels: Vec<Label>,
    pub end: Rect,
    pub short: bool,
}

impl Graph {
    pub(crate) fn stage(&self, stage: usize) -> impl Iterator<Item = &Card> {
        self.cards.iter().filter(move |c| c.stage == stage)
    }

    pub(crate) fn end_marker(&self) -> Spot {
        Spot { x: RAIL, y: self.end.mid_y() }
    }

    pub(crate) fn sources(&self, stage: usize) -> Vec<&str> {
        self.stage(stage).map(|c| c.role.as_str()).collect()
    }

    pub(crate) fn carries(&self, wire: &Wire, step: &Step) -> bool {
        let sources = match step.edge {
            Some(Edge::Forward { stage }) => self.sources(stage),
            _ => Vec::new(),
        };
        wire.carries(step, &sources)
    }
}

#[derive(Debug, Clone, Copy)]
struct Lane {
    from: usize,
    to: usize,
    x: f32,
    rise: f32,
    shift: f32,
}

impl Lane {
    fn crosses(self, stage: usize, x: f32) -> bool {
        self.to == stage || (self.to <= stage && stage <= self.from && self.x < x)
    }
}

pub(crate) fn float(n: usize) -> f32 {
    u16::try_from(n).map_or(f32::from(u16::MAX), f32::from)
}

fn row_width(roles: usize, card: f32) -> f32 {
    float(roles) * card + float(roles.saturating_sub(1)) * GAP_X
}

fn stage_top(first: f32, stage: usize) -> f32 {
    first + float(stage) * (CARD_H + STAGE_GAP)
}

#[cfg(test)]
pub(crate) fn layout(flow: &FlowSpec, outside: Option<Edge>) -> Graph {
    shape(flow, outside, CARD_W, false)
}

pub(crate) fn fit(flow: &FlowSpec, outside: Option<Edge>, room: f32) -> Graph {
    for short in [false, true] {
        let natural = shape(flow, outside, CARD_W, short);
        if natural.width <= room {
            return natural;
        }
    }
    let (mut narrow, mut wide) = (MIN_CARD, CARD_W);
    for _ in 0..8 {
        let card = f32::midpoint(narrow, wide);
        if shape(flow, outside, card, true).width <= room {
            narrow = card;
        } else {
            wide = card;
        }
    }
    shape(flow, outside, narrow, true)
}

fn shape(flow: &FlowSpec, outside: Option<Edge>, card: f32, short: bool) -> Graph {
    let column = flow.stages.iter().map(|s| row_width(s.roles.len(), card)).fold(LEAD_W.max(END_W), f32::max);
    let centre = COLUMN + column / 2.;
    let divider = LEAD_H + DIVIDER;
    let first = divider + FIRST;
    let cards = place_cards(flow, centre, first, card);
    let stages = flow.stages.len();
    let end_top = if stages == 0 { first } else { stage_top(first, stages - 1) + CARD_H + END_GAP };
    let mut graph = Graph {
        lead: Rect { x: centre - LEAD_W / 2., y: 0., w: LEAD_W, h: LEAD_H },
        divider,
        column: (COLUMN, COLUMN + column),
        markers: (0..stages).map(|i| Spot { x: RAIL, y: stage_top(first, i) + CARD_H / 2. }).collect(),
        end: Rect { x: centre - END_W / 2., y: end_top, w: END_W, h: END_H },
        cards,
        short,
        ..Graph::default()
    };
    let lanes = lanes(flow, &graph);
    let wires = forward_wires(&graph, stages)
        .into_iter()
        .chain(end_wires(&graph, stages))
        .chain(lanes.iter().filter_map(|lane| loop_wire(&graph, *lane)))
        .chain(outside.and_then(|edge| outside_wire(&graph, edge)))
        .collect();
    graph.wires = wires;
    graph.labels = labels(flow, &graph, &lanes);
    graph.width = width(flow, &graph, &lanes);
    graph.height = graph.end.bottom() + MARGIN;
    graph
}

fn place_cards(flow: &FlowSpec, centre: f32, first: f32, card: f32) -> Vec<Card> {
    flow.stages
        .iter()
        .enumerate()
        .flat_map(|(stage, spec)| {
            let left = centre - row_width(spec.roles.len(), card) / 2.;
            spec.roles.iter().enumerate().map(move |(j, role)| Card {
                role: role.clone(),
                stage,
                rect: Rect { x: left + float(j) * (card + GAP_X), y: stage_top(first, stage), w: card, h: CARD_H },
            })
        })
        .collect()
}

fn down_wire(edge: Option<Edge>, from: (Option<&str>, Spot), to: (Option<&str>, Spot)) -> Wire {
    let tip = Spot { x: to.1.x, y: to.1.y - NEAR };
    let start = Spot { x: from.1.x, y: from.1.y + NEAR };
    Wire {
        edge,
        from: from.0.map(str::to_string),
        to: to.0.map(str::to_string),
        curves: vec![Curve::drop(start, Spot { x: tip.x, y: tip.y - HEAD })],
        tip,
        way: Way::Down,
    }
}

fn forward_wires(graph: &Graph, stages: usize) -> Vec<Wire> {
    let pairs = (1..stages)
        .flat_map(|next| graph.stage(next - 1).flat_map(move |s| graph.stage(next).map(move |t| (next - 1, s, t))));
    pairs
        .map(|(stage, s, t)| {
            let from = Spot { x: s.rect.centre_x(), y: s.rect.bottom() };
            let to = Spot { x: t.rect.centre_x(), y: t.rect.y };
            down_wire(Some(Edge::Forward { stage }), (Some(&s.role), from), (Some(&t.role), to))
        })
        .collect()
}

fn end_wires(graph: &Graph, stages: usize) -> Vec<Wire> {
    let end = Spot { x: graph.end.centre_x(), y: graph.end.y };
    let last = stages.checked_sub(1).map(|stage| graph.stage(stage).collect::<Vec<_>>()).unwrap_or_default();
    last.into_iter()
        .map(|card| {
            let from = Spot { x: card.rect.centre_x(), y: card.rect.bottom() };
            down_wire(None, (Some(&card.role), from), (None, end))
        })
        .collect()
}

fn span(graph: &Graph, from: usize, to: usize) -> impl Iterator<Item = &Card> {
    (to.min(from)..=to.max(from)).flat_map(|s| graph.stage(s))
}

fn span_right(graph: &Graph, from: usize, to: usize) -> f32 {
    span(graph, from, to).map(|c| c.rect.right()).fold(0., f32::max)
}

fn span_left(graph: &Graph, from: Option<usize>, to: usize) -> f32 {
    let lead = if from.is_none() { graph.lead.x } else { f32::MAX };
    span(graph, from.unwrap_or_default(), to).map(|c| c.rect.x).fold(lead, f32::min)
}

fn lanes(flow: &FlowSpec, graph: &Graph) -> Vec<Lane> {
    let mut loops: Vec<(usize, usize)> = flow
        .stages
        .iter()
        .enumerate()
        .filter_map(|(from, stage)| Some((from, stage.back.as_ref()?.to.checked_sub(1)?)))
        .filter(|(from, to)| to < from && graph.stage(*from).next().is_some() && graph.stage(*to).next().is_some())
        .collect();
    loops.sort_by_key(|(from, to)| (from - to, *from));
    let mut placed: Vec<Lane> = Vec::new();
    for (from, to) in loops {
        let overlapping = placed.iter().filter(|l| l.to <= from && to <= l.from).map(|l| l.x + LANE);
        let x = overlapping.fold(span_right(graph, from, to) + LANE, f32::max);
        placed.push(Lane { from, to, x, rise: 0., shift: 0. });
    }
    ports(&placed)
}

fn ports(lanes: &[Lane]) -> Vec<Lane> {
    lanes
        .iter()
        .map(|lane| Lane { rise: port(lanes, lane.from, None), shift: port(lanes, lane.to, Some(lane.x)), ..*lane })
        .collect()
}

fn port(lanes: &[Lane], stage: usize, arriving: Option<f32>) -> f32 {
    let leaves = usize::from(lanes.iter().any(|l| l.from == stage));
    let arrivals: Vec<f32> = lanes.iter().filter(|l| l.to == stage).map(|l| l.x).collect();
    let count = leaves + arrivals.len();
    let index = arriving.map_or(0, |x| leaves + arrivals.iter().filter(|other| **other > x).count());
    (float(index) - float(count.saturating_sub(1)) / 2.) * SPREAD
}

fn bracket(a: Spot, b: Spot, x: f32) -> Vec<Curve> {
    let out = (x - a.x).signum();
    let back = (b.x - x).signum();
    let down = (b.y - a.y).signum();
    let p1 = Spot { x: x - out * BEND, y: a.y };
    let p2 = Spot { x, y: a.y + down * BEND };
    let p3 = Spot { x, y: b.y - down * BEND };
    let p4 = Spot { x: x + back * BEND, y: b.y };
    vec![
        Curve::line(a, p1),
        Curve::corner(p1, p2, true),
        Curve::line(p2, p3),
        Curve::corner(p3, p4, false),
        Curve::line(p4, b),
    ]
}

fn outermost(graph: &Graph, stage: usize, right: bool) -> Option<&Card> {
    let mut cards = graph.stage(stage);
    if right { cards.last() } else { cards.next() }
}

fn loop_wire(graph: &Graph, lane: Lane) -> Option<Wire> {
    let source = outermost(graph, lane.from, true)?;
    let target = outermost(graph, lane.to, true)?;
    let start = Spot { x: source.rect.right() + NEAR, y: source.rect.mid_y() + lane.rise };
    let tip = Spot { x: target.rect.right() + NEAR, y: target.rect.mid_y() + lane.shift };
    Some(Wire {
        edge: Some(Edge::Loop { stage: lane.from }),
        from: Some(source.role.clone()),
        to: Some(target.role.clone()),
        curves: bracket(start, Spot { x: tip.x + HEAD, y: tip.y }, lane.x),
        tip,
        way: Way::Left,
    })
}

fn outside_wire(graph: &Graph, edge: Edge) -> Option<Wire> {
    let Edge::Outside { from, to } = edge else { return None };
    let target = outermost(graph, to, false)?;
    let (role, start) = match from {
        Some(stage) => {
            let card = outermost(graph, stage, false)?;
            (Some(card.role.clone()), Spot { x: card.rect.x - NEAR, y: card.rect.mid_y() })
        }
        None => (None, Spot { x: graph.lead.x - NEAR, y: graph.lead.mid_y() }),
    };
    let tip = Spot { x: target.rect.x - NEAR, y: target.rect.mid_y() };
    let lane = span_left(graph, from, to) - LANE;
    Some(Wire {
        edge: Some(edge),
        from: role,
        to: Some(target.role.clone()),
        curves: bracket(start, Spot { x: tip.x - HEAD, y: tip.y }, lane.max(RAIL + LANE / 2.)),
        tip,
        way: Way::Right,
    })
}

fn labels(flow: &FlowSpec, graph: &Graph, lanes: &[Lane]) -> Vec<Label> {
    let beyond = lanes.iter().map(|l| l.x).fold(0., f32::max) + LABEL_GAP;
    lanes
        .iter()
        .filter_map(|lane| {
            let width = label_width(flow.stages.get(lane.from)?.back.as_ref()?, graph.short);
            let start = outermost(graph, lane.from, true)?.rect.right() + NEAR;
            let room = lane.x - BEND - start;
            let crossed = lanes.iter().any(|l| l.from != lane.from && l.crosses(lane.from, lane.x));
            let x = if !crossed && room >= width + 2. * LABEL_GAP { start + (room - width) / 2. } else { beyond };
            Some(Label { stage: lane.from, at: Spot { x, y: graph.markers.get(lane.from)?.y + lane.rise } })
        })
        .collect()
}

pub(crate) fn label_width(back: &BackSpec, short: bool) -> f32 {
    float(back.label(None, short).chars().count()) * CHAR_W + LABEL_PAD
}

fn width(flow: &FlowSpec, graph: &Graph, lanes: &[Lane]) -> f32 {
    let labels = graph.labels.iter().filter_map(|label| {
        let back = flow.stages.get(label.stage)?.back.as_ref()?;
        Some(label.at.x + label_width(back, graph.short))
    });
    let lanes = lanes.iter().map(|l| l.x + BEND);
    labels.chain(lanes).fold(graph.column.1, f32::max) + MARGIN
}

#[cfg(test)]
mod tests;
