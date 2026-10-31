use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    App, Bounds, Context, Corners, CursorStyle, ElementInputHandler, Entity, FocusHandle, Hitbox, HitboxBehavior,
    InteractiveElement as _, IntoElement, ParentElement as _, Pixels, Point, RenderImage, Styled as _, WeakEntity,
    Window, canvas, div, point, px, size,
};
use serde_json::{Value, json};

use super::Browser;
use super::devices::Device;
use super::frame::{Meta, Shot};
use super::pointer::{self, Area};
use crate::theme::{colours, radius};

const SETTLE: Duration = Duration::from_millis(150);

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct View {
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) scale: f32,
    pub(crate) device: Device,
}

impl View {
    pub(crate) fn measured(bounds: Bounds<Pixels>, scale: f32) -> Self {
        Self { width: f32::from(bounds.size.width), height: f32::from(bounds.size.height), scale, ..Self::default() }
    }

    pub(crate) fn known(&self) -> bool {
        self.width >= 1. && self.height >= 1. && self.scale > 0.
    }

    pub(crate) fn metrics(&self) -> Option<Value> {
        self.device.metrics(self)
    }

    pub(crate) fn device_pixels(&self) -> (i64, i64) {
        self.device.frame_pixels(self)
    }
}

pub(crate) fn fit(bounds: Bounds<Pixels>, meta: &Meta) -> Bounds<Pixels> {
    let (bw, bh) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    if meta.width <= 0. || meta.height <= 0. {
        return bounds;
    }
    let scale = (bw / meta.width).min(bh / meta.height).min(1.);
    let (w, h) = (meta.width * scale, meta.height * scale);
    let x = bounds.origin.x + px(((bw - w) / 2.).max(0.));
    let y = bounds.origin.y + px(((bh - h) / 2.).max(0.));
    Bounds::new(point(x, y), size(px(w), px(h)))
}

pub(crate) fn cursor_style(css: &str) -> CursorStyle {
    match css.split(',').next_back().unwrap_or(css).trim() {
        "pointer" => CursorStyle::PointingHand,
        "text" => CursorStyle::IBeam,
        "vertical-text" => CursorStyle::IBeamCursorForVerticalLayout,
        "crosshair" => CursorStyle::Crosshair,
        "grab" | "move" | "all-scroll" => CursorStyle::OpenHand,
        "grabbing" => CursorStyle::ClosedHand,
        "not-allowed" | "no-drop" => CursorStyle::OperationNotAllowed,
        "col-resize" => CursorStyle::ResizeColumn,
        "row-resize" => CursorStyle::ResizeRow,
        "ew-resize" | "e-resize" | "w-resize" => CursorStyle::ResizeLeftRight,
        "ns-resize" | "n-resize" | "s-resize" => CursorStyle::ResizeUpDown,
        "nwse-resize" | "nw-resize" | "se-resize" => CursorStyle::ResizeUpLeftDownRight,
        "nesw-resize" | "ne-resize" | "sw-resize" => CursorStyle::ResizeUpRightDownLeft,
        "context-menu" => CursorStyle::ContextualMenu,
        "copy" => CursorStyle::DragCopy,
        "alias" => CursorStyle::DragLink,
        _ => CursorStyle::Arrow,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Rect {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) w: f32,
    pub(crate) h: f32,
}

#[expect(clippy::cast_possible_truncation, reason = "page coordinates in CSS pixels fit an f32")]
fn number(value: &Value, key: &str) -> Option<f32> {
    value.get(key).and_then(Value::as_f64).map(|n| n as f32)
}

impl Rect {
    pub(crate) fn read(value: &Value) -> Option<Self> {
        let (x, y) = (number(value, "x")?, number(value, "y")?);
        Some(Self { x, y, w: number(value, "w").unwrap_or(0.), h: number(value, "h").unwrap_or(0.) })
    }
}

fn scale(shown: Bounds<Pixels>, meta: &Meta) -> f32 {
    if meta.width > 0. { f32::from(shown.size.width) / meta.width } else { 1. }
}

pub(crate) fn page_point(at: Point<Pixels>, bounds: Bounds<Pixels>, meta: &Meta) -> (f32, f32) {
    let shown = fit(bounds, meta);
    let scale = scale(shown, meta);
    (f32::from(at.x - shown.origin.x) / scale, f32::from(at.y - shown.origin.y) / scale)
}

pub(crate) fn to_page(at: Point<Pixels>, bounds: Bounds<Pixels>, meta: &Meta) -> Option<(f32, f32)> {
    fit(bounds, meta).contains(&at).then(|| page_point(at, bounds, meta))
}

pub(crate) fn from_page(rect: Rect, bounds: Bounds<Pixels>, meta: &Meta) -> Bounds<Pixels> {
    let shown = fit(bounds, meta);
    let scale = scale(shown, meta);
    let origin = point(shown.origin.x + px(rect.x * scale), shown.origin.y + px(rect.y * scale));
    Bounds::new(origin, size(px(rect.w * scale), px(rect.h * scale)))
}

impl Browser {
    pub(super) fn viewport(&self, _window: &Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let (entity, wanted) = (cx.entity().downgrade(), self.view);
        let painting = self.painting(cx);
        div()
            .id("browser-viewport")
            .size_full()
            .bg(colours::pane())
            .cursor(self.cursor)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::page_key_down))
            .on_key_up(cx.listener(Self::page_key_up))
            .child(
                canvas(
                    move |bounds, window, cx| {
                        measure(&entity, wanted, View::measured(bounds, window.scale_factor()), cx);
                        window.insert_hitbox(bounds, HitboxBehavior::Normal)
                    },
                    move |bounds, hitbox, window, cx| painting.paint(bounds, &hitbox, window, cx),
                )
                .size_full(),
            )
            .children(self.choice_menu(cx))
    }

    fn painting(&self, cx: &Context<Self>) -> Painting {
        Painting {
            stale: self.stale.clone(),
            shot: self.tabs.active(self.workspace).and_then(|t| t.shot.clone()),
            entity: cx.entity(),
            focus: self.focus.clone(),
            dragging: self.input.pointer.dragging().then_some(self.cursor),
            menu_open: self.input.choice.is_some(),
            bounds: self.input.bounds.clone(),
        }
    }

    pub(super) fn resize(&mut self, view: View, cx: &mut Context<Self>) {
        let view = View { device: self.view.device, ..view };
        if view == self.view {
            return;
        }
        let first = !self.view.known();
        self.view = view;
        self.resizing += 1;
        if first {
            self.apply_size();
            return;
        }
        let serial = self.resizing;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SETTLE).await;
            let _ = this.update(cx, |this, _| (this.resizing == serial).then(|| this.apply_size()));
        })
        .detach();
    }

    pub(super) fn apply_size(&self) {
        let Some(session) = self.casting.as_deref().and_then(|t| self.tabs.get(t)).and_then(|t| t.session.clone())
        else {
            return;
        };
        self.size_page(&session);
        self.send(Some(&session), "Page.stopScreencast", json!({}));
        self.start_cast(&session);
    }
}

fn measure(entity: &WeakEntity<Browser>, wanted: View, view: View, cx: &mut App) {
    if (View { device: wanted.device, ..view }) == wanted {
        return;
    }
    let entity = entity.clone();
    cx.defer(move |cx| {
        let _ = entity.update(cx, |this, cx| this.resize(view, cx));
    });
}

struct Painting {
    stale: Rc<RefCell<Vec<Arc<RenderImage>>>>,
    shot: Option<Shot>,
    entity: Entity<Browser>,
    focus: FocusHandle,
    dragging: Option<CursorStyle>,
    menu_open: bool,
    bounds: Rc<Cell<Bounds<Pixels>>>,
}

impl Painting {
    fn paint(self, bounds: Bounds<Pixels>, hitbox: &Hitbox, window: &mut Window, cx: &mut App) {
        for image in self.stale.borrow_mut().drain(..) {
            let _ = window.drop_image(image);
        }
        self.bounds.set(bounds);
        window.handle_input(&self.focus, ElementInputHandler::new(bounds, self.entity.clone()), cx);
        if let Some(cursor) = self.dragging {
            window.set_window_cursor_style(cursor);
        }
        let Some(shot) = self.shot else { return };
        let corners = Corners { bottom_left: px(radius::CARD), bottom_right: px(radius::CARD), ..Corners::default() };
        let _ = window.paint_image(fit(bounds, &shot.meta), corners, shot.image.clone(), 0, false);
        let entity = self.entity.downgrade();
        let area = Area {
            entity,
            focus: self.focus,
            hitbox: hitbox.clone(),
            bounds,
            meta: shot.meta,
            menu_open: self.menu_open,
        };
        pointer::listen(&area, window);
    }
}

#[cfg(test)]
mod tests;
