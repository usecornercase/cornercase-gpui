use gpui::{App, Bounds, DisplayId, Pixels, Size, Window, WindowBounds, point, px, size};
use serde::{Deserialize, Serialize};

use super::{TITLEBAR_HEIGHT, frame};
use crate::platform::Os;

pub(crate) const SHOWN: (f32, f32) = (1280., 800.);
pub(crate) const LEAST: (f32, f32) = (640., 360.);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Placed {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) display: Option<String>,
    pub(crate) bounds: Rect,
    #[serde(default)]
    pub(crate) state: State,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub(crate) struct Rect {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum State {
    Maximised,
    FullScreen,
    #[default]
    #[serde(other)]
    Windowed,
}

impl From<Bounds<Pixels>> for Rect {
    fn from(bounds: Bounds<Pixels>) -> Self {
        Self {
            x: f32::from(bounds.origin.x),
            y: f32::from(bounds.origin.y),
            width: f32::from(bounds.size.width),
            height: f32::from(bounds.size.height),
        }
    }
}

impl Rect {
    pub(crate) fn bounds(self) -> Bounds<Pixels> {
        Bounds::new(point(px(self.x), px(self.y)), size(px(self.width), px(self.height)))
    }
}

pub(crate) struct Screen {
    pub(crate) uuid: Option<String>,
    pub(crate) bounds: Bounds<Pixels>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum At {
    Centred((f32, f32)),
    On { screen: usize, bounds: Bounds<Pixels> },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Opening {
    pub(crate) at: At,
    pub(crate) state: State,
    pub(crate) width: f32,
}

fn usable(saved: &Placed) -> bool {
    let b = saved.bounds;
    [b.x, b.y, b.width, b.height].iter().all(|v| v.is_finite()) && b.width > 0. && b.height > 0.
}

pub(crate) fn fits(bounds: Bounds<Pixels>, screen: Bounds<Pixels>) -> bool {
    let inside = bounds.size.width <= screen.size.width && bounds.size.height <= screen.size.height;
    let title_row = bounds.top() >= screen.top() && bounds.top() + px(TITLEBAR_HEIGHT) <= screen.bottom();
    let centre = bounds.center();
    inside && title_row && screen.contains(&centre)
}

fn screen_of(saved: &Placed, screens: &[Screen], os: Os) -> Option<usize> {
    if os == Os::Linux {
        return None;
    }
    let display = saved.display.as_ref()?;
    let bounds = saved.bounds.bounds();
    screens.iter().position(|s| s.uuid.as_ref() == Some(display) && fits(bounds, s.bounds))
}

fn room(state: State, windowed: f32, screen: Option<Pixels>) -> f32 {
    match (state, screen) {
        (State::Windowed, _) | (_, None) => windowed,
        (_, Some(width)) => f32::from(width),
    }
}

fn first(primary: Option<Size<Pixels>>) -> Opening {
    let shown = frame::opening_size(SHOWN, LEAST, primary);
    Opening { at: At::Centred(shown), state: State::Windowed, width: shown.0 }
}

pub(crate) fn opening(saved: Option<&Placed>, screens: &[Screen], primary: Option<Size<Pixels>>, os: Os) -> Opening {
    let Some(saved) = saved.filter(|s| usable(s)) else { return first(primary) };
    let state = saved.state;
    if let Some(screen) = screen_of(saved, screens, os) {
        let bounds = saved.bounds.bounds();
        let width = room(state, f32::from(bounds.size.width), Some(screens[screen].bounds.size.width));
        return Opening { at: At::On { screen, bounds }, state, width };
    }
    let wanted = (saved.bounds.width, saved.bounds.height);
    let shown = frame::opening_size(wanted, LEAST, primary);
    let width = room(state, shown.0, primary.map(|p| p.width));
    Opening { at: At::Centred(shown), state, width }
}

pub(crate) fn keep(before: Option<&Placed>, seen: Placed) -> Placed {
    match before {
        Some(before) if seen.state != State::Windowed => Placed { state: seen.state, ..before.clone() },
        Some(before) => Placed { display: seen.display.or_else(|| before.display.clone()), ..seen },
        None => seen,
    }
}

fn wrap(state: State, bounds: Bounds<Pixels>) -> WindowBounds {
    match state {
        State::Windowed => WindowBounds::Windowed(bounds),
        State::Maximised => WindowBounds::Maximized(bounds),
        State::FullScreen => WindowBounds::Fullscreen(bounds),
    }
}

pub(crate) struct Options {
    pub(crate) bounds: WindowBounds,
    pub(crate) display: Option<DisplayId>,
    pub(crate) width: f32,
    pub(crate) opened: Placed,
}

fn opened(opening: &Opening, screens: &[Screen], cx: &App) -> Placed {
    let (bounds, uuid) = match &opening.at {
        At::Centred(shown) => {
            let uuid = cx.primary_display().and_then(|d| d.uuid().ok()).map(|u| u.to_string());
            (Bounds::centered(None, size(px(shown.0), px(shown.1)), cx), uuid)
        }
        At::On { screen, bounds } => (*bounds, screens[*screen].uuid.clone()),
    };
    Placed { display: uuid, bounds: Rect::from(bounds), state: State::Windowed }
}

pub(crate) fn window_options(saved: Option<&Placed>, cx: &App) -> Options {
    let displays = cx.displays();
    let screens: Vec<Screen> =
        displays.iter().map(|d| Screen { uuid: d.uuid().ok().map(|u| u.to_string()), bounds: d.bounds() }).collect();
    let primary = cx.primary_display().map(|d| d.bounds().size);
    let opening = opening(saved, &screens, primary, Os::CURRENT);
    let placed = opened(&opening, &screens, cx);
    let (bounds, display) = match opening.at {
        At::Centred(shown) => (Bounds::centered(None, frame::outer(Os::CURRENT, shown), cx), None),
        At::On { screen, bounds } => (bounds, displays.get(screen).map(|d| d.id())),
    };
    Options { bounds: wrap(opening.state, bounds), display, width: opening.width, opened: placed }
}

pub(crate) fn seen(window: &Window, cx: &App) -> Placed {
    let state = if window.is_fullscreen() {
        State::FullScreen
    } else if window.is_maximized() {
        State::Maximised
    } else {
        State::Windowed
    };
    Placed {
        display: window.display(cx).and_then(|d| d.uuid().ok()).map(|u| u.to_string()),
        bounds: Rect::from(window.inner_window_bounds().get_bounds()),
        state,
    }
}

#[cfg(test)]
mod tests;
