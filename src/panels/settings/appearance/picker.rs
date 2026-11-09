use gpui::{
    AppContext as _, Context, Corner, Div, Entity, InteractiveElement as _, MouseButton, ParentElement as _, Rgba,
    StatefulInteractiveElement as _, Styled as _, Window, anchored, deferred, div, point, px,
};

use super::Snap;
use crate::appearance::{Appearance, Part};
use crate::panels::settings::SettingsPanel;
use crate::theme::colour::hex;
use crate::theme::tokens::{ANSI_NAMES, OPAQUE};
use crate::ui;
use crate::ui::colour_picker::{ColourPicker, Picked};

pub(in crate::panels::settings) struct OpenPicker {
    pub(super) key: String,
    pub(super) part: Part,
    pub(super) view: Entity<ColourPicker>,
}

fn quick_picks(snap: &Snap) -> Vec<(String, Rgba)> {
    let terminal = &snap.theme.terminal;
    let named = [("foreground", terminal.foreground), ("background", terminal.background)];
    let ansi = ANSI_NAMES.iter().zip(terminal.ansi);
    named
        .into_iter()
        .chain(ansi.map(|(name, bytes)| (*name, bytes)))
        .map(|(name, bytes)| (name.replace('_', " "), crate::theme::colour::from_bytes(bytes)))
        .collect()
}

pub(super) struct Spot<'a> {
    pub key: &'a str,
    pub part: Part,
    pub colour: Rgba,
    pub size: f32,
}

impl SettingsPanel {
    pub(in crate::panels::settings) fn close_picker(&mut self, cx: &mut Context<Self>) {
        if self.look.picker.take().is_some() {
            cx.notify();
        }
    }

    fn toggle_picker(
        &mut self,
        spot: &Spot<'_>,
        quick: Vec<(String, Rgba)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let open = self.look.picker.as_ref().is_some_and(|p| p.key == spot.key && p.part == spot.part);
        self.look.picker = None;
        if !open {
            let with_alpha = spot.part == Part::Chrome && !OPAQUE.contains(&spot.key);
            let colour = spot.colour;
            let view = cx.new(|cx| ColourPicker::new(colour, with_alpha, quick, window, cx));
            let (key, part) = (spot.key.to_string(), spot.part);
            cx.subscribe(&view, move |this, _, event: &Picked, cx| this.picked(&key, part, event, cx)).detach();
            self.look.picker = Some(OpenPicker { key: spot.key.to_string(), part: spot.part, view });
        }
        cx.notify();
    }

    fn picked(&mut self, key: &str, part: Part, event: &Picked, cx: &mut Context<Self>) {
        match event {
            Picked::Changed(colour) => {
                let colour = *colour;
                Appearance::get(cx).update(cx, |appearance, cx| appearance.set_colour(part, key, Some(colour), cx));
            }
            Picked::Closed => self.close_picker(cx),
        }
    }

    pub(super) fn swatch_button(&self, spot: &Spot<'_>, snap: &Snap, cx: &mut Context<Self>) -> Div {
        let open = self.look.picker.as_ref().filter(|p| p.key == spot.key && p.part == spot.part);
        let (key, part, colour, size) = (spot.key.to_string(), spot.part, spot.colour, spot.size);
        let quick = quick_picks(snap);
        let tooltip = format!("{} {}", spot.key.replace('_', " "), hex(colour, colour.a < 1.));
        let button = ui::swatch(colour, size)
            .id(gpui::SharedString::from(format!("swatch-{part:?}-{key}")))
            .cursor_pointer()
            .tooltip(crate::notify::tip(tooltip))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    let spot = Spot { key: &key, part, colour, size };
                    this.toggle_picker(&spot, quick.clone(), window, cx);
                }),
            );
        let popup = open.map(|open| {
            let placed = anchored()
                .anchor(Corner::TopLeft)
                .offset(point(px(0.), px(size + 6.)))
                .snap_to_window_with_margin(px(8.));
            div().absolute().top_0().left_0().child(deferred(placed.child(open.view.clone())).with_priority(1))
        });
        div().relative().child(button).children(popup)
    }
}
