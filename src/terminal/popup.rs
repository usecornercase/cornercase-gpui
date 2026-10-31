use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AnyElement, App, AppContext as _, Bounds, ClickEvent, Context, Entity, FocusHandle, Global, InteractiveElement,
    IntoElement, MouseButton, MouseDownEvent, MouseUpEvent, ParentElement, Pixels, Point, Render, ScrollWheelEvent,
    StatefulInteractiveElement as _, Styled, WeakEntity, Window, canvas, div, px,
};
use serde_json::{Value, json};

use super::mouse::{button_name, mouse, scroll_steps, wheel_event};
use crate::cli::Stream;
use crate::model::{Applied, Frame, Grid, MouseMode, OpenPopup, PaneGrid};
use crate::render::Metrics;
use crate::render::layout::Layout;
use crate::render::popup::{HEADER, INSET, MARGIN, content_size, fitted, paint_popup};
use crate::store::{self, Store, args};
use crate::theme::{colours, radius};
use crate::ui;

const RETRY: Duration = Duration::from_millis(500);
const RETRY_FAILED: Duration = Duration::from_secs(5);

pub(crate) struct Popup {
    store: Entity<Store>,
    terminal: FocusHandle,
    open: Option<OpenPopup>,
    stream: Option<Stream>,
    generation: u64,
    grid: Rc<Grid>,
    shown: bool,
    error: Option<String>,
    metrics: Option<Metrics>,
    room: Rc<Cell<(f32, f32)>>,
    layout: Rc<RefCell<Layout>>,
    scroll_rest: f32,
}

struct GlobalPopup(Entity<Popup>);
impl Global for GlobalPopup {}

pub(crate) fn input_args(pane: u64, event: &Value) -> Vec<String> {
    args(&["input", "--popup", "--pane", &pane.to_string(), &event.to_string()])
}

pub(crate) fn title(open: &OpenPopup) -> String {
    format!("{} · {}", open.action, open.plugin)
}

fn retry_after(error: &str, last: Option<&str>) -> Duration {
    if !error.is_empty() && last == Some(error) { RETRY_FAILED } else { RETRY }
}

fn takes_release(pane: &PaneGrid) -> bool {
    !matches!(pane.modes.mouse, MouseMode::None | MouseMode::Press)
}

impl Popup {
    pub(crate) fn get(terminal: &FocusHandle, cx: &mut App) -> Entity<Popup> {
        let store = Store::get(cx);
        if let Some(GlobalPopup(popup)) = cx.try_global::<GlobalPopup>()
            && popup.read(cx).store == store
        {
            return popup.clone();
        }
        let terminal = terminal.clone();
        let popup = cx.new(|cx| Popup::new(store, terminal, cx));
        cx.set_global(GlobalPopup(popup.clone()));
        popup
    }

    fn new(store: Entity<Store>, terminal: FocusHandle, cx: &mut Context<Self>) -> Self {
        cx.observe(&store, |this, _, cx| this.app_changed(cx)).detach();
        let mut this = Self {
            store,
            terminal,
            open: None,
            stream: None,
            generation: 0,
            grid: Rc::default(),
            shown: false,
            error: None,
            metrics: None,
            room: Rc::default(),
            layout: Rc::default(),
            scroll_rest: 0.,
        };
        this.app_changed(cx);
        this
    }

    pub(crate) fn is_open(&self) -> bool {
        self.open.is_some()
    }

    fn pane(&self) -> Option<&PaneGrid> {
        let id = self.grid.popup.as_ref().filter(|_| self.shown && self.open.is_some())?.pane;
        self.grid.pane(id)
    }

    pub(crate) fn send(&self, event: &Value, cx: &App) {
        if let Some(pane) = self.pane() {
            self.store.read(cx).cmd(input_args(pane.pane, event));
        }
    }

    pub(crate) fn paste(&self, cx: &App) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.send(&json!({ "paste": text }), cx);
        }
    }

    pub(crate) fn close(&self, cx: &App) {
        self.store.read(cx).cmd(args(&["plugin", "popup-close"]));
    }

    pub(crate) fn set_metrics(&mut self, metrics: &Metrics, cx: &mut Context<Self>) {
        if self.metrics.as_ref().map(|m| m.font_size) != Some(metrics.font_size) {
            self.metrics = Some(metrics.clone());
            cx.notify();
        }
    }

    fn app_changed(&mut self, cx: &mut Context<Self>) {
        let open = OpenPopup::in_app(self.store.read(cx).app.as_ref());
        if open == self.open {
            return;
        }
        self.open = open;
        if self.open.is_none() {
            self.reset();
        } else if self.stream.is_none() {
            self.subscribe(cx);
        }
        cx.notify();
    }

    fn reset(&mut self) {
        self.generation += 1;
        self.stream = None;
        self.grid = Rc::default();
        self.shown = false;
        self.error = None;
    }

    fn subscribe(&mut self, cx: &mut Context<Self>) {
        self.generation += 1;
        let generation = self.generation;
        self.stream = store::follow_until(
            cx,
            args(&["surface", "--popup", "--patches"]),
            move |this, line, cx| this.line(generation, &line, cx),
            move |this, error, cx| this.ended(generation, &error, cx),
        );
    }

    fn line(&mut self, generation: u64, line: &str, cx: &mut Context<Self>) {
        if generation != self.generation {
            return;
        }
        let Ok(frame) = serde_json::from_str::<Frame>(line) else { return };
        if let Applied::NeedRedraw = Rc::make_mut(&mut self.grid).apply(frame) {
            let surface = self.grid.surface.to_string();
            self.store.read(cx).cmd(args(&["redraw", "--surface", &surface]));
            return;
        }
        self.shown = true;
        self.error = None;
        cx.notify();
    }

    fn ended(&mut self, generation: u64, error: &str, cx: &mut Context<Self>) {
        if generation != self.generation {
            return;
        }
        let error = error.lines().last().unwrap_or_default().trim().to_string();
        let wait = retry_after(&error, self.error.as_deref());
        self.reset();
        self.error = (!error.is_empty()).then_some(error);
        let generation = self.generation;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            let _ = this.update(cx, |this, cx| this.resubscribe(generation, cx));
        })
        .detach();
        cx.notify();
    }

    fn resubscribe(&mut self, generation: u64, cx: &mut Context<Self>) {
        if self.generation == generation && self.stream.is_none() && self.open.is_some() {
            self.subscribe(cx);
        }
    }

    fn cell_at(&self, at: Point<Pixels>) -> Option<(PaneGrid, (i32, i32))> {
        let pane = self.pane()?.clone();
        let cell = self.layout.borrow().cell_in(pane.pane, at)?;
        Some((pane, cell))
    }

    fn on_mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.terminal);
        let Some((pane, (x, y))) = self.cell_at(event.position) else { return };
        if pane.modes.mouse != MouseMode::None {
            self.send(&mouse("down", button_name(event.button), pane.clamp_cell(x, y)), cx);
        }
    }

    fn on_mouse_up(&mut self, event: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some((pane, (x, y))) = self.cell_at(event.position) else { return };
        if takes_release(&pane) {
            self.send(&mouse("up", button_name(event.button), pane.clamp_cell(x, y)), cx);
        }
    }

    fn on_scroll(&mut self, event: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(line) = self.metrics.as_ref().map(|m| m.line_height) else { return };
        self.scroll_rest += f32::from(event.delta.pixel_delta(line).y) / f32::from(line);
        let whole = self.scroll_rest.trunc();
        if whole.abs() < 1. {
            return;
        }
        self.scroll_rest -= whole;
        let Some((pane, at)) = self.cell_at(event.position) else { return };
        let Some(wheel) = wheel_event(&pane, at, whole > 0.) else { return };
        for _ in 0..scroll_steps(whole) {
            self.send(&wheel, cx);
        }
    }
}

impl Popup {
    fn room_probe(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let (room, entity) = (self.room.clone(), cx.entity().downgrade());
        canvas(move |bounds, _, cx| measured(&room, bounds, entity, cx), |_, (), _, _| {})
            .absolute()
            .top_0()
            .left_0()
            .size_full()
    }

    fn card(&self, open: &OpenPopup, window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        ui::glass::card(radius::SHEET)
            .id("plugin-popup")
            .flex()
            .flex_col()
            .overflow_hidden()
            .occlude()
            .child(Self::header(open, cx))
            .child(div().p(px(INSET)).child(self.body(window, cx)))
    }

    fn body(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(metrics) = self.metrics.clone().filter(|_| self.shown) else {
            return self.waiting().into_any_element();
        };
        let (width, height) = fitted(content_size(&self.grid, metrics.cell()), self.room.get());
        let focused = self.terminal.is_focused(window) && window.is_window_active();
        let (grid, layout) = (self.grid.clone(), self.layout.clone());
        let paint = move |bounds: Bounds<Pixels>, (): (), window: &mut Window, cx: &mut App| {
            layout.replace(paint_popup(&grid, (&metrics, focused), bounds, window, cx));
        };
        div()
            .w(px(width))
            .h(px(height))
            .overflow_hidden()
            .child(canvas(|_, _, _| {}, paint).size_full())
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Right, cx.listener(Self::on_mouse_up))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .into_any_element()
    }

    fn header(open: &OpenPopup, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .h(px(HEADER))
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .pl(px(16.))
            .pr(px(10.))
            .border_b_1()
            .border_color(colours::border())
            .child(div().text_size(px(12.)).text_color(colours::soft()).child(title(open)))
            .child(
                ui::icon_button("plugin-popup-close", "✕")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.close(cx))),
            )
    }

    fn waiting(&self) -> impl IntoElement + use<> {
        let text = self.error.as_ref().map_or_else(
            || "Opening the popup…".to_string(),
            |why| format!("The popup cannot be shown: {why}. Retrying…"),
        );
        div().w(px(360.)).p(px(16.)).text_size(px(12.)).text_color(colours::muted()).child(text)
    }
}

fn measured(room: &Rc<Cell<(f32, f32)>>, bounds: Bounds<Pixels>, entity: WeakEntity<Popup>, cx: &mut App) {
    let size = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    if room.get() != size {
        room.set(size);
        cx.defer(move |cx| {
            let _ = entity.update(cx, |_, cx| cx.notify());
        });
    }
}

impl Render for Popup {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(open) = self.open.clone() else { return div().into_any_element() };
        div()
            .absolute()
            .inset_0()
            .p(px(MARGIN))
            .occlude()
            .bg(colours::veil())
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, window, _| {
                    window.focus(&this.terminal);
                }),
            )
            .child(self.room_probe(cx))
            .child(self.card(&open, window, cx))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests;
