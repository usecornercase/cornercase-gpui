use std::ops::Range;

use gpui::{
    AnyElement, App, AvailableSpace, Bounds, Element, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    ParentElement as _, Pixels, Point, Size, Style, Window, div, point, px, size,
};

const SLACK: f32 = 1.;

type Build = Box<dyn Fn(Fit) -> AnyElement>;
type Drawn = Box<dyn FnOnce(Range<usize>, &mut App)>;
type Typing = Box<dyn Fn(&Window) -> Option<Bounds<Pixels>>>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Fit {
    pub(crate) most: f32,
    pub(crate) measuring: bool,
    pub(crate) capped: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Fitting {
    pub(crate) shown: usize,
    pub(crate) more: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Standing {
    pub(crate) top: f32,
    pub(crate) bottom: f32,
    pub(crate) from_top: bool,
}

pub(crate) struct Stack {
    gap: f32,
    items: Vec<Build>,
    more: Option<Box<dyn Fn(usize) -> AnyElement>>,
    on_drawn: Option<Drawn>,
    clear_of: Option<ClearOf>,
}

struct ClearOf {
    typing: Typing,
    clear: f32,
    least: f32,
}

#[derive(Clone, Copy)]
struct Room {
    bounds: Bounds<Pixels>,
    standing: Standing,
}

struct Placed {
    most: f32,
    height: f32,
    capped: bool,
}

pub(crate) fn stack(gap: f32) -> Stack {
    Stack { gap, items: Vec::new(), more: None, on_drawn: None, clear_of: None }
}

impl Stack {
    pub(crate) fn item(mut self, build: impl Fn(Fit) -> AnyElement + 'static) -> Self {
        self.items.push(Box::new(build));
        self
    }

    pub(crate) fn more(mut self, build: impl Fn(usize) -> AnyElement + 'static) -> Self {
        self.more = Some(Box::new(build));
        self
    }

    pub(crate) fn on_drawn(mut self, drawn: impl FnOnce(Range<usize>, &mut App) + 'static) -> Self {
        self.on_drawn = Some(Box::new(drawn));
        self
    }

    pub(crate) fn clear_of(
        mut self,
        typing: impl Fn(&Window) -> Option<Bounds<Pixels>> + 'static,
        clear: f32,
        least: f32,
    ) -> Self {
        self.clear_of = Some(ClearOf { typing: Box::new(typing), clear, least });
        self
    }

    fn room(&self, bounds: Bounds<Pixels>, window: &Window) -> Room {
        let column = (f32::from(bounds.top()), f32::from(bounds.bottom()));
        let sideways = (f32::from(bounds.left()), f32::from(bounds.right()));
        let standing = match &self.clear_of {
            None => standing(column, None, 0., 0.),
            Some(clear_of) => {
                let field = (clear_of.typing)(window)
                    .filter(|field| across(sideways, (f32::from(field.left()), f32::from(field.right()))))
                    .map(|field| (f32::from(field.top()), f32::from(field.bottom())));
                standing(column, field, clear_of.clear, clear_of.least)
            }
        };
        let height = px((standing.bottom - standing.top).max(0.));
        Room { bounds: Bounds::new(point(bounds.left(), px(standing.top)), size(bounds.size.width, height)), standing }
    }

    fn line(&self, width: Pixels, window: &mut Window, cx: &mut App) -> f32 {
        self.more.as_ref().map_or(0., |more| measure(more(self.items.len()), width, window, cx))
    }

    fn place(&self, newest: usize, (most, width): (f32, Pixels), window: &mut Window, cx: &mut App) -> Placed {
        let build = &self.items[newest];
        cut_to(most, |fit| measure(build(fit), width, window, cx))
    }

    fn placed(&self, bounds: Bounds<Pixels>, line: f32, window: &mut Window, cx: &mut App) -> Vec<Placed> {
        let most = most(f32::from(bounds.size.height), self.items.len(), self.gap, line);
        let beside = (most, bounds.size.width);
        (0..self.items.len()).rev().map(|at| self.place(at, beside, window, cx)).collect()
    }

    fn draw(&self, placed: &[Placed], room: Room, window: &mut Window, cx: &mut App) -> (Vec<AnyElement>, f32) {
        let heights: Vec<f32> = placed.iter().map(|one| one.height).collect();
        let (tops, free) = tops(&heights, self.gap, room.standing);
        let mut drawn = Vec::new();
        for (at, (one, top)) in placed.iter().zip(tops).enumerate() {
            let build = &self.items[self.items.len() - 1 - at];
            let item = build(Fit { most: one.most, measuring: false, capped: one.capped });
            drawn.push(prepaint(item, point(room.bounds.left(), px(top)), room.bounds.size.width, window, cx));
        }
        (drawn, free)
    }
}

pub(crate) fn across(column: (f32, f32), field: (f32, f32)) -> bool {
    field.1 > column.0 && field.0 < column.1
}

pub(crate) fn standing(column: (f32, f32), field: Option<(f32, f32)>, clear: f32, least: f32) -> Standing {
    let whole = Standing { top: column.0, bottom: column.1, from_top: false };
    let Some((field_top, field_bottom)) = field.filter(|field| across(column, *field)) else { return whole };
    let above = (field_top - clear - column.0).max(0.);
    let below = (column.1 - field_bottom - clear).max(0.);
    if above.max(below) < least {
        whole
    } else if below >= above {
        Standing { top: column.1 - below, bottom: column.1, from_top: false }
    } else {
        Standing { top: column.0, bottom: column.0 + above, from_top: true }
    }
}

pub(crate) fn tops(newest_first: &[f32], gap: f32, standing: Standing) -> (Vec<f32>, f32) {
    let mut edge = if standing.from_top { standing.top } else { standing.bottom };
    let mut tops = Vec::new();
    for height in newest_first {
        let top = if standing.from_top { edge } else { edge - height };
        tops.push(top);
        edge = if standing.from_top { top + height + gap } else { top - gap };
    }
    (tops, edge)
}

pub(crate) fn fitting(newest_first: &[f32], gap: f32, room: f32, line: f32) -> Fitting {
    let tops: Vec<f32> = newest_first
        .iter()
        .scan(0., |sum, height| {
            *sum += height;
            Some(*sum)
        })
        .zip(0u16..)
        .map(|(sum, at)| sum + f32::from(at) * gap)
        .collect();
    let shown = tops.iter().take_while(|top| **top <= room + SLACK).count();
    if shown == newest_first.len() {
        return Fitting { shown, more: false };
    }
    let shown_with_line = tops.iter().take_while(|top| **top + gap + line <= room + SLACK).count();
    Fitting { shown: shown_with_line, more: shown_with_line > 0 || line <= room + SLACK }
}

pub(crate) fn most(room: f32, items: usize, gap: f32, line: f32) -> f32 {
    let reserve = if items > 1 { gap + line } else { 0. };
    (room - reserve).max(0.)
}

fn cut_to(most: f32, mut measure: impl FnMut(Fit) -> f32) -> Placed {
    let height = measure(Fit { most, measuring: true, capped: false });
    if height <= most {
        return Placed { most, height, capped: false };
    }
    let probe = most / 2.;
    let chrome = measure(Fit { most: probe, measuring: true, capped: true }) - probe;
    let cap = (most - chrome).max(0.);
    Placed { most: cap, height: measure(Fit { most: cap, measuring: true, capped: true }), capped: true }
}

fn available(width: Pixels) -> Size<AvailableSpace> {
    size(AvailableSpace::Definite(width), AvailableSpace::MinContent)
}

fn measure(item: AnyElement, width: Pixels, window: &mut Window, cx: &mut App) -> f32 {
    let mut root = div().child(item).into_any_element();
    f32::from(root.layout_as_root(available(width), window, cx).height)
}

fn prepaint(item: AnyElement, at: Point<Pixels>, width: Pixels, window: &mut Window, cx: &mut App) -> AnyElement {
    let mut root = div().child(item).into_any_element();
    root.prepaint_as_root(at, available(width), window, cx);
    root
}

impl IntoElement for Stack {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Stack {
    type RequestLayoutState = ();
    type PrepaintState = Vec<AnyElement>;

    super::anonymous_element!();

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let style = Style { size: Size::full(), ..Style::default() };
        (window.request_layout(style, None, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        (): &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let room = self.room(bounds, window);
        let width = room.bounds.size.width;
        let line = self.line(width, window, cx);
        let placed = self.placed(room.bounds, line, window, cx);
        let heights: Vec<f32> = placed.iter().map(|one| one.height).collect();
        let fitting = fitting(&heights, self.gap, f32::from(room.bounds.size.height), line);
        let (mut drawn, free) = self.draw(&placed[..fitting.shown], room, window, cx);
        let waiting = self.items.len() - fitting.shown;
        if let (true, Some(more)) = (fitting.more, self.more.as_ref()) {
            let at = if room.standing.from_top { free } else { free - line };
            drawn.push(prepaint(more(waiting), point(room.bounds.left(), px(at)), width, window, cx));
        }
        drawn.reverse();
        if let Some(on_drawn) = self.on_drawn.take() {
            on_drawn(waiting..self.items.len(), cx);
        }
        drawn
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        (): &mut Self::RequestLayoutState,
        drawn: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        for item in drawn {
            item.paint(window, cx);
        }
    }
}

#[cfg(test)]
mod tests;
