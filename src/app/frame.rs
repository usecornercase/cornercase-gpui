use std::time::Duration;

use gpui::{
    AnyElement, AnyWindowHandle, App, BoxShadow, ClickEvent, CursorStyle, Decorations, Div, Global, Hsla,
    InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, ParentElement as _, Pixels,
    ResizeEdge, Size, Stateful, StatefulInteractiveElement as _, Styled, Tiling, Window, WindowBackgroundAppearance,
    WindowBounds, WindowControls, WindowDecorations, div, point, prelude::FluentBuilder as _, px, size,
};

use super::GAP;
use crate::platform::Os;
use crate::theme::{colours, radius};
use crate::ui::{self, Icon};

pub(crate) const APP_ID: &str = "cornercase-gpui";
const RESTORE_POLL: Duration = Duration::from_millis(20);
const RESTORE_TRIES: u32 = 100;
const INSET: f32 = 12.;
const REACH_IN: f32 = 4.;
const CORNER_REACH: f32 = 16.;
const CORNER: f32 = radius::CARD + GAP;
const CONTROL: f32 = 24.;

pub(crate) fn decorations(os: Os) -> Option<WindowDecorations> {
    match os {
        Os::Mac => None,
        Os::Linux => Some(WindowDecorations::Client),
    }
}

pub(crate) fn outer(os: Os, (width, height): (f32, f32)) -> Size<Pixels> {
    let margin = if os == Os::Linux { 2. * INSET } else { 0. };
    size(px(width + margin), px(height + margin))
}

pub(crate) fn background(os: Os) -> WindowBackgroundAppearance {
    match os {
        Os::Mac => WindowBackgroundAppearance::Blurred,
        Os::Linux => WindowBackgroundAppearance::Transparent,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Sides {
    pub(crate) top: f32,
    pub(crate) right: f32,
    pub(crate) bottom: f32,
    pub(crate) left: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Frame {
    pub(crate) drawn: bool,
    tiling: Tiling,
    controls: WindowControls,
    maximised: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Control {
    Minimise,
    Maximise,
    Restore,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Restore {
    Wait,
    Resize,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Press {
    Zoom,
    Hold,
    Menu,
    Pass,
}

impl Frame {
    pub(crate) fn read(window: &Window) -> Self {
        Self::of(window.window_decorations(), window.window_controls(), window.is_maximized())
    }

    pub(crate) fn of(decorations: Decorations, controls: WindowControls, maximised: bool) -> Self {
        match decorations {
            Decorations::Server => Self { controls, maximised, ..Self::default() },
            Decorations::Client { tiling } => Self { drawn: true, tiling, controls, maximised },
        }
    }

    pub(crate) fn margins(self) -> Sides {
        let side = |tiled: bool| if self.drawn && !tiled { INSET } else { 0. };
        let t = self.tiling;
        Sides { top: side(t.top), right: side(t.right), bottom: side(t.bottom), left: side(t.left) }
    }

    pub(crate) fn corners(self) -> [f32; 4] {
        let m = self.margins();
        let round = |a: f32, b: f32| if a > 0. && b > 0. { CORNER } else { 0. };
        [round(m.top, m.left), round(m.top, m.right), round(m.bottom, m.right), round(m.bottom, m.left)]
    }

    pub(crate) fn floating(self) -> bool {
        self.drawn && !self.tiling.is_tiled()
    }

    pub(crate) fn grips(self) -> Vec<ResizeEdge> {
        let m = self.margins();
        [
            (ResizeEdge::Top, m.top),
            (ResizeEdge::Right, m.right),
            (ResizeEdge::Bottom, m.bottom),
            (ResizeEdge::Left, m.left),
            (ResizeEdge::TopLeft, m.top.min(m.left)),
            (ResizeEdge::TopRight, m.top.min(m.right)),
            (ResizeEdge::BottomRight, m.bottom.min(m.right)),
            (ResizeEdge::BottomLeft, m.bottom.min(m.left)),
        ]
        .into_iter()
        .filter(|(_, reach)| *reach > 0.)
        .map(|(edge, _)| edge)
        .collect()
    }

    pub(crate) fn buttons(self) -> Vec<Control> {
        if !self.drawn {
            return Vec::new();
        }
        let resize = if self.maximised { Control::Restore } else { Control::Maximise };
        [(Control::Minimise, self.controls.minimize), (resize, self.controls.maximize), (Control::Close, true)]
            .into_iter()
            .filter_map(|(control, shown)| shown.then_some(control))
            .collect()
    }
}

pub(crate) fn cursor(edge: ResizeEdge) -> CursorStyle {
    match edge {
        ResizeEdge::Top | ResizeEdge::Bottom => CursorStyle::ResizeUpDown,
        ResizeEdge::Left | ResizeEdge::Right => CursorStyle::ResizeLeftRight,
        ResizeEdge::TopLeft | ResizeEdge::BottomRight => CursorStyle::ResizeUpLeftDownRight,
        ResizeEdge::TopRight | ResizeEdge::BottomLeft => CursorStyle::ResizeUpRightDownLeft,
    }
}

pub(crate) fn press(button: MouseButton, clicks: usize, claimed: bool, menu: bool) -> Press {
    match (button, clicks) {
        _ if claimed => Press::Pass,
        (MouseButton::Left, 2) => Press::Zoom,
        (MouseButton::Left, _) => Press::Hold,
        (MouseButton::Right, _) if menu => Press::Menu,
        _ => Press::Pass,
    }
}

#[derive(Default)]
struct Grip {
    held: bool,
    claimed: bool,
}

impl Global for Grip {}

impl Grip {
    fn down(e: &MouseDownEvent, menu: bool, window: &Window, cx: &mut App) {
        let grip = cx.default_global::<Grip>();
        match press(e.button, e.click_count, std::mem::take(&mut grip.claimed), menu) {
            Press::Zoom => window.zoom_window(),
            Press::Hold => grip.held = true,
            Press::Menu => window.show_window_menu(e.position),
            Press::Pass => {}
        }
    }

    fn moved(e: &MouseMoveEvent, window: &mut Window, cx: &mut App) {
        let grip = cx.default_global::<Grip>();
        if grip.held && e.dragging() {
            grip.held = false;
            window.start_window_move();
        }
    }
}

impl Control {
    fn look(self) -> (&'static str, Icon) {
        match self {
            Control::Minimise => ("window-minimise", Icon::Minimise),
            Control::Maximise => ("window-maximise", Icon::Maximise),
            Control::Restore => ("window-restore", Icon::Restore),
            Control::Close => ("window-close", Icon::Close),
        }
    }

    fn act(self, window: &mut Window) {
        match self {
            Control::Minimise => window.minimize_window(),
            Control::Maximise | Control::Restore => window.zoom_window(),
            Control::Close => window.remove_window(),
        }
    }
}

fn control_button(control: Control) -> Stateful<Div> {
    let (id, glyph) = control.look();
    div()
        .id(id)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .size(px(CONTROL))
        .rounded_full()
        .bg(colours::hover())
        .hover(|s| s.bg(colours::selected()))
        .child(ui::icon(glyph, 16.).text_color(colours::soft()))
        .on_click(move |_: &ClickEvent, window, _| control.act(window))
}

pub(crate) fn round<E: Styled>(element: E, frame: Frame) -> E {
    if !frame.drawn {
        return element;
    }
    let [tl, tr, br, bl] = frame.corners();
    element.rounded_tl(px(tl)).rounded_tr(px(tr)).rounded_br(px(br)).rounded_bl(px(bl))
}

fn shadow() -> Vec<BoxShadow> {
    let dark = Hsla { h: 0., s: 0., l: 0., a: 0.5 };
    vec![BoxShadow { color: dark, offset: point(px(0.), px(1.)), blur_radius: px(INSET - 2.), spread_radius: px(0.) }]
}

fn window_card(content: Div, frame: Frame) -> Div {
    let m = frame.margins();
    round(content, frame)
        .overflow_hidden()
        .border_color(colours::border())
        .when(m.top > 0., Styled::border_t_1)
        .when(m.right > 0., Styled::border_r_1)
        .when(m.bottom > 0., Styled::border_b_1)
        .when(m.left > 0., Styled::border_l_1)
        .when(frame.floating(), |d| d.shadow(shadow()))
}

fn grip(edge: ResizeEdge, m: Sides) -> Div {
    let strip =
        div().absolute().occlude().cursor(cursor(edge)).on_mouse_down(MouseButton::Left, move |_, window, cx| {
            cx.stop_propagation();
            window.start_window_resize(edge);
        });
    let corner = |m: f32| px(m + CORNER_REACH);
    match edge {
        ResizeEdge::Top => strip.top_0().left_0().right_0().h(px(m.top + REACH_IN)),
        ResizeEdge::Bottom => strip.bottom_0().left_0().right_0().h(px(m.bottom + REACH_IN)),
        ResizeEdge::Left => strip.top_0().bottom_0().left_0().w(px(m.left + REACH_IN)),
        ResizeEdge::Right => strip.top_0().bottom_0().right_0().w(px(m.right + REACH_IN)),
        ResizeEdge::TopLeft => strip.top_0().left_0().size(corner(m.top)),
        ResizeEdge::TopRight => strip.top_0().right_0().size(corner(m.top)),
        ResizeEdge::BottomRight => strip.bottom_0().right_0().size(corner(m.bottom)),
        ResizeEdge::BottomLeft => strip.bottom_0().left_0().size(corner(m.bottom)),
    }
}

pub(crate) fn framed(content: Div, frame: Frame, window: &mut Window) -> AnyElement {
    if !frame.drawn {
        return content.into_any_element();
    }
    window.set_client_inset(px(INSET));
    let m = frame.margins();
    div()
        .size_full()
        .relative()
        .pt(px(m.top))
        .pr(px(m.right))
        .pb(px(m.bottom))
        .pl(px(m.left))
        .child(window_card(content, frame))
        .children(frame.grips().into_iter().map(|edge| grip(edge, m)))
        .into_any_element()
}

pub(crate) fn title_row(row: Div, frame: Frame) -> Div {
    if !frame.drawn {
        return row;
    }
    let menu = frame.controls.window_menu;
    row.on_any_mouse_down(move |e, window, cx| Grip::down(e, menu, window, cx))
        .on_mouse_move(Grip::moved)
        .on_mouse_up(MouseButton::Left, |_, _, cx| cx.default_global::<Grip>().held = false)
}

pub(crate) fn claimed(element: Div, frame: Frame) -> Div {
    if !frame.drawn {
        return element;
    }
    element.on_any_mouse_down(|_, _, cx| cx.default_global::<Grip>().claimed = true)
}

pub(crate) fn toggle_full_screen(window: &mut Window, cx: &mut App) {
    let windowed = windowed_size(Os::CURRENT, window.window_bounds());
    window.toggle_fullscreen();
    if let Some(size) = windowed {
        restore_when_out(window.window_handle(), size, cx);
    }
}

pub(crate) fn windowed_size(os: Os, bounds: WindowBounds) -> Option<Size<Pixels>> {
    match (os, bounds) {
        (Os::Linux, WindowBounds::Fullscreen(windowed)) => Some(windowed.size),
        _ => None,
    }
}

pub(crate) fn restore_step(full_screen: bool, frame: Frame, now: Size<Pixels>, windowed: Size<Pixels>) -> Restore {
    if full_screen {
        Restore::Wait
    } else if frame.maximised || frame.tiling.is_tiled() || now == windowed {
        Restore::Done
    } else {
        Restore::Resize
    }
}

fn restore(window: &mut Window, windowed: Size<Pixels>) -> Restore {
    let step = restore_step(window.is_fullscreen(), Frame::read(window), window.bounds().size, windowed);
    if step == Restore::Resize {
        window.resize(windowed);
    }
    step
}

fn restore_when_out(window: AnyWindowHandle, windowed: Size<Pixels>, cx: &mut App) {
    cx.spawn(async move |cx| {
        for _ in 0..RESTORE_TRIES {
            cx.background_executor().timer(RESTORE_POLL).await;
            if !matches!(window.update(cx, |_, window, _| restore(window, windowed)), Ok(Restore::Wait)) {
                break;
            }
        }
    })
    .detach();
}

pub(crate) fn window_controls(frame: Frame) -> Option<Div> {
    let buttons = frame.buttons();
    if buttons.is_empty() {
        return None;
    }
    let row = div().flex().flex_none().items_center().gap(px(10.)).pl(px(6.));
    Some(claimed(row.children(buttons.into_iter().map(control_button)), frame))
}

#[cfg(test)]
mod tests;
