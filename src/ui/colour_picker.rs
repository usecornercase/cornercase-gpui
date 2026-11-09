mod hsv;
mod strips;

use gpui::{
    AppContext as _, Context, DragMoveEvent, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, ParentElement as _, Render, Rgba, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use super::split::Blank;
use super::{TextField, TextFieldEvent, swatch, track};
use crate::theme::colour::{hex, opaque, parse_hex};
use crate::theme::{colours, radius};
use hsv::Hsv;
use strips::{Alpha, Hue, Square};

pub(crate) const WIDTH: f32 = 224.;

pub(crate) enum Picked {
    Changed(Rgba),
    Closed,
}

pub(crate) struct ColourPicker {
    hsv: Hsv,
    alpha: f32,
    with_alpha: bool,
    field: Entity<TextField>,
    error: Option<String>,
    quick: Vec<(String, Rgba)>,
    focus: FocusHandle,
}

impl EventEmitter<Picked> for ColourPicker {}

impl Focusable for ColourPicker {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl ColourPicker {
    pub(crate) fn new(
        colour: Rgba,
        with_alpha: bool,
        quick: Vec<(String, Rgba)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let field = cx.new(|cx| TextField::new("#rrggbb", cx).reverts_on_escape());
        let shown = hex(colour, with_alpha);
        field.update(cx, |field, cx| field.set_text(shown, cx));
        cx.subscribe(&field, |this, field, event: &TextFieldEvent, cx| {
            if matches!(event, TextFieldEvent::Changed | TextFieldEvent::Submit) {
                this.typed(&field.read(cx).text(), cx);
            }
        })
        .detach();
        let focus = cx.focus_handle();
        window.focus(&focus);
        let alpha = if with_alpha { colour.a } else { 1. };
        Self { hsv: Hsv::from_rgba(colour), alpha, with_alpha, field, error: None, quick, focus }
    }

    fn colour(&self) -> Rgba {
        self.hsv.to_rgba(self.alpha)
    }

    fn typed(&mut self, text: &str, cx: &mut Context<Self>) {
        match parse_hex(text.trim()) {
            Ok(colour) if hex(colour, self.with_alpha) == hex(self.colour(), self.with_alpha) => self.error = None,
            Ok(colour) => {
                self.error = None;
                self.hsv = Hsv::from_rgba(colour);
                self.alpha = if self.with_alpha { colour.a } else { 1. };
                cx.emit(Picked::Changed(self.colour()));
            }
            Err(error) => self.error = Some(error.to_string()),
        }
        cx.notify();
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        let shown = hex(self.colour(), self.with_alpha);
        self.field.update(cx, |field, cx| field.set_text(shown, cx));
        self.error = None;
        cx.emit(Picked::Changed(self.colour()));
        cx.notify();
    }

    fn pick(&mut self, colour: Rgba, cx: &mut Context<Self>) {
        let alpha = if self.with_alpha { self.alpha } else { 1. };
        self.hsv = Hsv::from_rgba(colour);
        self.alpha = alpha;
        self.changed(cx);
    }

    fn in_square(&mut self, at: gpui::Point<gpui::Pixels>, cx: &mut Context<Self>) {
        if let Some((x, y)) = track::pointer_along(strips::SQUARE, at) {
            self.hsv = Hsv { s: x, v: 1. - y, ..self.hsv };
            self.changed(cx);
        }
    }

    fn on_hue(&mut self, at: gpui::Point<gpui::Pixels>, cx: &mut Context<Self>) {
        if let Some((x, _)) = track::pointer_along(strips::HUE, at) {
            self.hsv = Hsv { h: x.min(0.999), ..self.hsv };
            self.changed(cx);
        }
    }

    fn on_alpha(&mut self, at: gpui::Point<gpui::Pixels>, cx: &mut Context<Self>) {
        if let Some((x, _)) = track::pointer_along(strips::ALPHA, at) {
            self.alpha = x;
            self.changed(cx);
        }
    }

    fn quick_row(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let swatches = self.quick.iter().enumerate().map(|(i, (name, colour))| {
            let colour = *colour;
            swatch(colour, 16.)
                .id(SharedString::from(format!("quick-{i}")))
                .cursor_pointer()
                .tooltip(crate::notify::tip(format!("{name} {}", hex(colour, false))))
                .on_click(cx.listener(move |this, _, _, cx| this.pick(colour, cx)))
        });
        div().flex().flex_wrap().gap(px(4.)).children(swatches)
    }

    fn hex_row(&self) -> impl IntoElement + use<> {
        let preview = swatch(self.colour(), 24.);
        let field = div().flex_1().min_w_0().child(super::input_box(self.field.clone(), false));
        div().flex().items_center().gap(px(8.)).child(preview).child(field)
    }
}

impl ColourPicker {
    fn square(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        Square::new(self.hsv)
            .draw()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, e: &MouseDownEvent, _, cx| this.in_square(e.position, cx)),
            )
            .on_drag_move(cx.listener(|this, e: &DragMoveEvent<Square>, _, cx| this.in_square(e.event.position, cx)))
    }

    fn hue(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        Hue::new(self.hsv)
            .draw()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, e: &MouseDownEvent, _, cx| this.on_hue(e.position, cx)),
            )
            .on_drag_move(cx.listener(|this, e: &DragMoveEvent<Hue>, _, cx| this.on_hue(e.event.position, cx)))
    }

    fn alpha_strip(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        self.with_alpha.then(|| {
            Alpha::new(opaque(self.colour()), self.alpha)
                .draw()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, e: &MouseDownEvent, _, cx| this.on_alpha(e.position, cx)),
                )
                .on_drag_move(cx.listener(|this, e: &DragMoveEvent<Alpha>, _, cx| this.on_alpha(e.event.position, cx)))
        })
    }
}

impl Render for ColourPicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let error = self.error.clone().map(super::error_line);
        crate::ui::glass::popover()
            .id("colour-picker")
            .track_focus(&self.focus)
            .key_context("ColourPicker")
            .w(px(WIDTH))
            .p(px(10.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .rounded(px(radius::card()))
            .occlude()
            .on_mouse_down(MouseButton::Left, |_: &MouseDownEvent, _, cx| cx.stop_propagation())
            .on_key_down(cx.listener(|_, e: &KeyDownEvent, _, cx| {
                if e.keystroke.key == "escape" {
                    cx.stop_propagation();
                    cx.emit(Picked::Closed);
                }
            }))
            .child(self.square(cx))
            .child(self.hue(cx))
            .children(self.alpha_strip(cx))
            .child(self.hex_row())
            .children(error)
            .child(self.quick_row(cx))
            .text_color(colours::text())
    }
}

pub(crate) fn blank_drag(cx: &mut gpui::App) -> Entity<Blank> {
    cx.new(|_| Blank)
}
