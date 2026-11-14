use std::ops::Range;

use gpui::{
    AnyElement, App, AvailableSpace, Bounds, Element, GlobalElementId, InspectorElementId, IntoElement, LayoutId,
    ParentElement as _, Pixels, Point, Size, Style, Window, div, point, px, size,
};

const SLACK: f32 = 1.;

type Build = Box<dyn Fn(Fit) -> AnyElement>;
type Drawn = Box<dyn FnOnce(Range<usize>, &mut App)>;

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

pub(crate) struct Stack {
    gap: f32,
    items: Vec<Build>,
    more: Option<Box<dyn Fn(usize) -> AnyElement>>,
    on_drawn: Option<Drawn>,
}

struct Placed {
    most: f32,
    height: f32,
    capped: bool,
}

pub(crate) fn stack(gap: f32) -> Stack {
    Stack { gap, items: Vec::new(), more: None, on_drawn: None }
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

    fn line(&self, width: Pixels, window: &mut Window, cx: &mut App) -> f32 {
        self.more.as_ref().map_or(0., |more| measure(more(self.items.len()), width, window, cx))
    }

    fn place(&self, newest: usize, (most, width): (f32, Pixels), window: &mut Window, cx: &mut App) -> Placed {
        let build = &self.items[newest];
        let fit = Fit { most, measuring: true, capped: false };
        let height = measure(build(fit), width, window, cx);
        match trimmed(most, height) {
            Some(most) => {
                let fit = Fit { most, measuring: true, capped: true };
                Placed { most, height: measure(build(fit), width, window, cx), capped: true }
            }
            None => Placed { most, height, capped: false },
        }
    }

    fn placed(&self, bounds: Bounds<Pixels>, line: f32, window: &mut Window, cx: &mut App) -> Vec<Placed> {
        let most = most(f32::from(bounds.size.height), self.items.len(), self.gap, line);
        let beside = (most, bounds.size.width);
        (0..self.items.len()).rev().map(|at| self.place(at, beside, window, cx)).collect()
    }

    fn draw(
        &self,
        placed: &[Placed],
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> (Vec<AnyElement>, f32) {
        let mut drawn = Vec::new();
        let mut bottom = bounds.bottom();
        for (at, one) in placed.iter().enumerate() {
            let build = &self.items[self.items.len() - 1 - at];
            let item = build(Fit { most: one.most, measuring: false, capped: one.capped });
            let at = point(bounds.left(), bottom - px(one.height));
            drawn.push(prepaint(item, at, bounds.size.width, window, cx));
            bottom -= px(one.height + self.gap);
        }
        (drawn, f32::from(bottom))
    }
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

pub(crate) fn trimmed(most: f32, measured: f32) -> Option<f32> {
    (measured > most).then(|| (2. * most - measured).max(0.))
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
        let width = bounds.size.width;
        let line = self.line(width, window, cx);
        let placed = self.placed(bounds, line, window, cx);
        let heights: Vec<f32> = placed.iter().map(|one| one.height).collect();
        let fitting = fitting(&heights, self.gap, f32::from(bounds.size.height), line);
        let (mut drawn, top) = self.draw(&placed[..fitting.shown], bounds, window, cx);
        let waiting = self.items.len() - fitting.shown;
        if let (true, Some(more)) = (fitting.more, self.more.as_ref()) {
            let above = prepaint(more(waiting), point(bounds.left(), px(top - line)), width, window, cx);
            drawn.push(above);
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
