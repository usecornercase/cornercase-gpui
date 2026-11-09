use gpui::{
    AnyElement, Context, Div, IntoElement as _, ParentElement as _, SharedString, StatefulInteractiveElement as _, div,
};

use super::{Snap, widgets};
use crate::appearance::Appearance;
use crate::panels::settings::SettingsPanel;
use crate::theme::tokens::Radius;
use crate::ui;

fn caption(radius: Radius) -> &'static str {
    match radius {
        Radius::Compact => "Compact",
        Radius::Default => "Default",
        Radius::Round => "Round",
    }
}

impl SettingsPanel {
    fn radius_row(snap: &Snap, cx: &mut Context<Self>) -> Div {
        let items: Vec<AnyElement> = Radius::ALL
            .into_iter()
            .map(|radius| {
                ui::segment(
                    SharedString::from(format!("radius-{}", radius.key())),
                    caption(radius),
                    snap.theme.shape.radius == radius,
                )
                .on_click(cx.listener(move |_, _, _, cx| {
                    Appearance::get(cx).update(cx, |appearance, cx| appearance.set_radius(radius, cx));
                }))
                .into_any_element()
            })
            .collect();
        widgets::row(
            true,
            widgets::label("Corners", "How round cards, buttons and fields are."),
            ui::segments(items),
            div(),
        )
    }

    fn glow_row(snap: &Snap, cx: &mut Context<Self>) -> Div {
        let on = snap.theme.shape.glow;
        let switch = widgets::switch("appearance-glow", on).on_click(cx.listener(move |_, _, _, cx| {
            Appearance::get(cx).update(cx, |appearance, cx| appearance.set_glow(!on, cx));
        }));
        widgets::row(
            false,
            widgets::label("Glow", "The cobalt halo around primary buttons and active cards."),
            switch,
            div(),
        )
    }

    pub(super) fn shape_group(snap: &Snap, cx: &mut Context<Self>) -> Div {
        widgets::group("Shape", widgets::card().child(Self::radius_row(snap, cx)).child(Self::glow_row(snap, cx)))
    }
}
