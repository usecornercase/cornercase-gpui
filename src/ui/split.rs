use gpui::{
    App, AppContext as _, Bounds, ClickEvent, Context, Div, DragMoveEvent, ElementId, InteractiveElement as _,
    IntoElement, Pixels, Point, Render, Stateful, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

const GRIP: f32 = 8.;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Split {
    default: f32,
    min: f32,
    max: f32,
    chosen: Option<f32>,
}

pub(crate) type Access<V> = fn(&mut V) -> &mut Split;

impl Split {
    pub(crate) const fn new(default: f32, min: f32, max: f32) -> Self {
        Self { default, min, max, chosen: None }
    }

    pub(crate) fn size(&self) -> f32 {
        self.chosen.unwrap_or(self.default)
    }

    pub(crate) fn chosen(&self) -> Option<f32> {
        self.chosen
    }

    fn drag(&mut self, wanted: f32, room: f32) -> bool {
        let size = wanted.clamp(self.min, room.min(self.max).max(self.min)).round();
        let changed = self.chosen != Some(size);
        self.chosen = Some(size);
        changed
    }

    fn follow(&mut self, edge: Edge, bounds: Bounds<Pixels>, pointer: Point<Pixels>, keep: f32) -> bool {
        self.drag(reach(edge, bounds, pointer), extent(edge, bounds) - keep)
    }

    fn reset(&mut self) -> bool {
        self.chosen.take().is_some()
    }
}

fn reach(edge: Edge, bounds: Bounds<Pixels>, pointer: Point<Pixels>) -> f32 {
    f32::from(match edge {
        Edge::Right => pointer.x - bounds.left(),
        Edge::Left => bounds.right() - pointer.x,
        Edge::Top => bounds.bottom() - pointer.y,
        Edge::Bottom => pointer.y - bounds.top(),
    })
}

fn extent(edge: Edge, bounds: Bounds<Pixels>) -> f32 {
    f32::from(match edge {
        Edge::Left | Edge::Right => bounds.size.width,
        Edge::Top | Edge::Bottom => bounds.size.height,
    })
}

struct Blank;

impl Render for Blank {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        gpui::Empty
    }
}

pub(crate) fn follow<V: 'static, T: 'static>(
    edge: Edge,
    keep: f32,
    split: Access<V>,
    cx: &Context<V>,
) -> impl Fn(&DragMoveEvent<T>, &mut Window, &mut App) + 'static {
    cx.listener(move |this, e: &DragMoveEvent<T>, _, cx| {
        if split(this).follow(edge, e.bounds, e.event.position, keep) {
            cx.notify();
        }
    })
}

pub(crate) fn handle<V: 'static, T: 'static>(
    id: impl Into<ElementId>,
    grip: T,
    (edge, at): (Edge, f32),
    split: Access<V>,
    cx: &Context<V>,
) -> Stateful<Div> {
    let start = px(at - GRIP / 2.);
    let reset = cx.listener(move |this, e: &ClickEvent, _, cx| {
        if e.click_count() == 2 && split(this).reset() {
            cx.notify();
        }
    });
    let handle =
        div().id(id).absolute().occlude().on_drag(grip, |_, _, _, cx: &mut App| cx.new(|_| Blank)).on_click(reset);
    match edge {
        Edge::Right => handle.top_0().bottom_0().left(start).w(px(GRIP)).cursor_col_resize(),
        Edge::Left => handle.top_0().bottom_0().right(start).w(px(GRIP)).cursor_col_resize(),
        Edge::Top => handle.left_0().right_0().bottom(start).h(px(GRIP)).cursor_row_resize(),
        Edge::Bottom => handle.left_0().right_0().top(start).h(px(GRIP)).cursor_row_resize(),
    }
}

#[cfg(test)]
mod tests;
