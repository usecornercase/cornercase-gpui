use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AnyElement, App, AvailableSpace, Bounds, Context, DispatchPhase, Div, ElementId, InteractiveElement as _,
    IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels, Point,
    Stateful, Styled as _, Task, TextLayout, WeakEntity, Window, canvas, div, point, px,
};

use crate::desktop;
use crate::platform::Os;
use crate::theme::{colours, radius};
use crate::ui::glass;
use crate::ui::one_line::LaidOut;

use super::code::past_the_cut;
use super::{
    Caret, Edge, Lit, Press, Selection, Zone, bar_origin, edge_at, host, off_centre, row_at, row_near, word_at,
};

const DRAG_AFTER: f32 = 3.;
const SCROLL_EVERY: Duration = Duration::from_millis(50);

enum DrawnText {
    Line(Box<LaidOut>),
    Wrapped(TextLayout, Option<usize>),
}

struct Drawn {
    bounds: Bounds<Pixels>,
    len: usize,
    text: DrawnText,
}

fn wrapped_index(layout: &TextLayout, at: Point<Pixels>) -> usize {
    let Some(line) = layout.line_layout_for_index(0) else { return 0 };
    let (height, origin) = (layout.line_height(), layout.bounds().origin);
    let rows = line.wrap_boundaries().len() + 1;
    let y = (at.y - origin.y).clamp(px(0.), height * rows - px(1.));
    line.closest_index_for_position(point(at.x - origin.x, y), height).unwrap_or_else(|index| index)
}

impl Drawn {
    fn caret(&self, at: Point<Pixels>) -> usize {
        if at.x < self.bounds.left() {
            return 0;
        }
        let index = match &self.text {
            DrawnText::Line(line) => line.index_at(at.x),
            DrawnText::Wrapped(layout, kept) => past_the_cut(wrapped_index(layout, at), *kept, self.len),
        };
        index.min(self.len)
    }

    fn top_of(&self, at: usize) -> Pixels {
        match &self.text {
            DrawnText::Line(_) => self.bounds.top(),
            DrawnText::Wrapped(layout, kept) => {
                let at = kept.map_or(at, |kept| at.min(kept));
                layout.position_for_index(at).map_or(self.bounds.top(), |spot| spot.y)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Off {
    Gone,
    InView,
    By(Pixels),
}

struct Frame<S> {
    rows: BTreeMap<S, Drawn>,
    body: Bounds<Pixels>,
}

pub(crate) struct Laid<S>(Rc<RefCell<Frame<S>>>);

impl<S> Clone for Laid<S> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<S> Default for Laid<S> {
    fn default() -> Self {
        Self(Rc::new(RefCell::new(Frame { rows: BTreeMap::new(), body: Bounds::default() })))
    }
}

impl<S: Copy + Ord> Laid<S> {
    pub(crate) fn clear(&self) {
        self.0.borrow_mut().rows.clear();
    }

    pub(crate) fn add_line(&self, spot: S, len: usize, line: LaidOut) {
        let drawn = Drawn { bounds: line.bounds, len, text: DrawnText::Line(Box::new(line)) };
        self.0.borrow_mut().rows.insert(spot, drawn);
    }

    fn add_wrapped(&self, (spot, len, kept): (S, usize, Option<usize>), bounds: Bounds<Pixels>, layout: TextLayout) {
        self.0.borrow_mut().rows.insert(spot, Drawn { bounds, len, text: DrawnText::Wrapped(layout, kept) });
    }

    fn set_body(&self, body: Bounds<Pixels>) {
        self.0.borrow_mut().body = body;
    }

    pub(crate) fn body(&self) -> Bounds<Pixels> {
        self.0.borrow().body
    }

    pub(crate) fn off(&self, spot: S, at: usize, inset: f32) -> Off {
        let frame = self.0.borrow();
        let Some(drawn) = frame.rows.get(&spot) else { return Off::Gone };
        off_centre(drawn.top_of(at), frame.body, px(inset)).map_or(Off::InView, Off::By)
    }

    fn pressed(&self, at: Point<Pixels>) -> Option<(Caret<S>, Zone)> {
        let frame = self.0.borrow();
        let spot = row_at(frame.rows.iter().map(|(spot, drawn)| (*spot, drawn.bounds)), at.y)?;
        let drawn = frame.rows.get(&spot)?;
        let zone = if at.x < drawn.bounds.left() { Zone::Numbers } else { Zone::Code };
        Some((Caret { spot, at: drawn.caret(at) }, zone))
    }

    fn near(&self, at: Point<Pixels>) -> Option<Caret<S>> {
        let frame = self.0.borrow();
        let spot = row_near(frame.rows.iter().map(|(spot, drawn)| (*spot, drawn.bounds)), at.y)?;
        Some(Caret { spot, at: frame.rows.get(&spot)?.caret(at) })
    }

    pub(crate) fn drawn_ends(&self) -> Option<(S, S)> {
        let frame = self.0.borrow();
        Some((*frame.rows.keys().next()?, *frame.rows.keys().next_back()?))
    }

    fn edge(&self, at: Point<Pixels>) -> Option<Edge> {
        edge_at(at.y, self.0.borrow().body)
    }

    fn float(
        &self,
        (selection, mut bar): (Selection<S>, AnyElement),
        inset: f32,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        let frame = self.0.borrow();
        let row = frame.rows.get(&host(selection, frame.rows.keys().copied())?)?.bounds;
        let mut room = frame.body;
        room.origin.y += px(inset);
        room.size.height -= px(inset);
        let size = bar.layout_as_root(AvailableSpace::min_size(), window, cx);
        bar.prepaint_at(bar_origin(row, size, room), window, cx);
        Some(bar)
    }
}

struct Pressing<S> {
    press: Press<S>,
    down_at: Point<Pixels>,
    pointer: Point<Pixels>,
    dragging: bool,
    _tick: Task<()>,
}

pub(crate) struct Selecting<S> {
    pub(crate) selection: Option<Selection<S>>,
    laid: Laid<S>,
    pressing: Option<Pressing<S>>,
}

impl<S> Default for Selecting<S> {
    fn default() -> Self {
        Self { selection: None, laid: Laid::default(), pressing: None }
    }
}

impl<S: Copy + Ord> Selecting<S> {
    pub(crate) fn new(selection: Option<Selection<S>>) -> Self {
        Self { selection, ..Self::default() }
    }

    pub(crate) fn laid(&self) -> &Laid<S> {
        &self.laid
    }

    pub(crate) fn lit(&self, spot: S, line: &str) -> Lit {
        self.selection.map_or(Lit::Nothing, |selection| selection.lit(spot, line))
    }

    fn follow(&mut self) -> bool {
        let Some(pressing) = self.pressing.as_mut() else { return false };
        let Some(to) = self.laid.near(pressing.pointer) else { return false };
        let selection = Some(pressing.press.dragged(to));
        let changed = self.selection != selection;
        self.selection = selection;
        changed
    }
}

pub(crate) trait Selects: Sized + 'static {
    type Spot: Copy + Ord + 'static;

    fn selecting(&mut self) -> Option<&mut Selecting<Self::Spot>>;
    fn scroll_a_row(&mut self, edge: Edge);
    fn line_text(&self, spot: Self::Spot) -> Option<&str>;
    fn selected_text(&self) -> Option<String>;
}

type Started<S> = (Press<S>, Option<Selection<S>>);

fn start<T: Selects>(
    this: &T,
    before: Option<Selection<T::Spot>>,
    (caret, zone): (Caret<T::Spot>, Zone),
    e: &MouseDownEvent,
) -> Started<T::Spot> {
    let shift = e.modifiers.shift;
    let word = (e.click_count == 2 && zone == Zone::Code && !shift)
        .then(|| this.line_text(caret.spot).map(|line| word_at(line, caret.at)))
        .flatten()
        .filter(|word| !word.is_empty());
    if let Some(word) = word {
        let (press, selected) = Press::on_word(before, caret.spot, word);
        return (press, Some(selected));
    }
    let zone = if e.click_count >= 3 { Zone::Numbers } else { zone };
    let before = before.filter(|_| e.click_count < 2 || shift);
    let press = Press::down(before, caret, zone, shift);
    (press, press.at_once())
}

pub(crate) fn body<T: Selects>(cx: &mut Context<T>) -> Div {
    div()
        .flex_1()
        .min_h_0()
        .relative()
        .bg(colours::terminal())
        .on_mouse_down(MouseButton::Left, cx.listener(|this, e: &MouseDownEvent, _, cx| mouse_down(this, e, cx)))
}

fn mouse_down<T: Selects>(this: &mut T, e: &MouseDownEvent, cx: &mut Context<T>) {
    let Some(selecting) = this.selecting() else { return };
    let before = selecting.selection;
    let Some(hit) = selecting.laid.pressed(e.position) else {
        selecting.selection = None;
        cx.notify();
        return;
    };
    let (press, selection) = start(this, before, hit, e);
    let tick = ticker(cx);
    let Some(selecting) = this.selecting() else { return };
    selecting.selection = selection;
    let (down_at, pointer) = (e.position, e.position);
    selecting.pressing = Some(Pressing { press, down_at, pointer, dragging: false, _tick: tick });
    cx.notify();
}

fn ticker<T: Selects>(cx: &mut Context<T>) -> Task<()> {
    cx.spawn(async move |this, cx| {
        loop {
            cx.background_executor().timer(SCROLL_EVERY).await;
            if !matches!(this.update(cx, ticked), Ok(true)) {
                return;
            }
        }
    })
}

fn ticked<T: Selects>(this: &mut T, cx: &mut Context<T>) -> bool {
    let Some(selecting) = this.selecting() else { return false };
    let Some(pressing) = selecting.pressing.as_ref() else { return false };
    if !pressing.dragging {
        return true;
    }
    if let Some(edge) = selecting.laid.edge(pressing.pointer) {
        this.scroll_a_row(edge);
        cx.notify();
    }
    if this.selecting().is_some_and(Selecting::follow) {
        cx.notify();
    }
    true
}

fn far_enough(from: Point<Pixels>, to: Point<Pixels>) -> bool {
    let (x, y) = (f32::from(to.x - from.x), f32::from(to.y - from.y));
    x.hypot(y) >= DRAG_AFTER
}

fn pointer_moved<T: Selects>(this: &mut T, position: Point<Pixels>, cx: &mut Context<T>) {
    let Some(selecting) = this.selecting() else { return };
    let Some(pressing) = selecting.pressing.as_mut() else { return };
    pressing.pointer = position;
    pressing.dragging |= far_enough(pressing.down_at, position);
    if pressing.dragging && selecting.follow() {
        cx.notify();
    }
}

fn released<T: Selects>(this: &mut T, cx: &mut Context<T>) {
    let Some(selecting) = this.selecting() else { return };
    if selecting.pressing.as_ref().is_some_and(|pressing| pressing.dragging) {
        selecting.follow();
    }
    let Some(pressing) = selecting.pressing.take() else { return };
    selecting.selection = pressing.press.released(selecting.selection);
    cx.notify();
    if pressing.press.moved()
        && desktop::has_primary(Os::CURRENT)
        && let Some(text) = this.selected_text()
    {
        desktop::write_primary(&text, cx);
    }
}

fn pointer_event<T: Selects>(view: &WeakEntity<T>, position: Option<Point<Pixels>>, cx: &mut App) {
    let _ = view.update(cx, |this, cx| match position {
        Some(position) => pointer_moved(this, position, cx),
        None => released(this, cx),
    });
}

fn listen<T: Selects>(view: WeakEntity<T>, window: &mut Window) {
    let moved = view.clone();
    window.on_mouse_event(move |e: &MouseMoveEvent, phase, _, cx| {
        if phase == DispatchPhase::Capture {
            pointer_event(&moved, e.dragging().then_some(e.position), cx);
        }
    });
    window.on_mouse_event(move |e: &MouseUpEvent, phase, _, cx| {
        if phase == DispatchPhase::Capture && e.button == MouseButton::Left {
            pointer_event(&view, None, cx);
        }
    });
}

pub(crate) fn layer<T: Selects>(
    view: WeakEntity<T>,
    selecting: &Selecting<T::Spot>,
    bar: Option<AnyElement>,
    inset: f32,
) -> impl IntoElement + use<T> {
    let (laid, pressing) = (selecting.laid.clone(), selecting.pressing.is_some());
    let floating = selecting.selection.zip(bar).filter(|_| !pressing);
    canvas(
        move |bounds, window, cx| {
            laid.set_body(bounds);
            floating.and_then(|floating| laid.float(floating, inset, window, cx))
        },
        move |_, bar, window, cx| {
            if let Some(mut bar) = bar {
                bar.paint(window, cx);
            }
            listen(view, window);
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

pub(super) fn row_probe<S: Copy + Ord + 'static>(
    laid: &Laid<S>,
    line: (S, usize, Option<usize>),
    layout: TextLayout,
) -> impl IntoElement + use<S> {
    let laid = laid.clone();
    canvas(move |bounds, _, _| laid.add_wrapped(line, bounds, layout), |_, (), _, _| {})
        .absolute()
        .top_0()
        .left_0()
        .size_full()
}

pub(crate) fn action(id: impl Into<ElementId>, label: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .px(px(6.))
        .rounded(px(radius::chip()))
        .text_size(px(11.))
        .text_color(colours::muted())
        .cursor_pointer()
        .whitespace_nowrap()
        .hover(|s| s.bg(colours::selected()).text_color(colours::text()))
        .on_mouse_down(MouseButton::Left, keep_selection)
        .child(label)
}

pub(crate) fn keep_selection(_: &MouseDownEvent, _: &mut Window, cx: &mut App) {
    cx.stop_propagation();
}

pub(crate) fn action_bar(label: String, actions: Vec<AnyElement>) -> Div {
    glass::popover()
        .occlude()
        .flex()
        .items_center()
        .gap(px(2.))
        .px(px(6.))
        .py(px(4.))
        .child(div().pr(px(4.)).text_size(px(11.)).text_color(colours::faint()).whitespace_nowrap().child(label))
        .children(actions)
}

#[cfg(test)]
mod tests;
