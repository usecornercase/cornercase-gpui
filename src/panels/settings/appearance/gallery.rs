use gpui::{
    AnyElement, Context, Div, InteractiveElement as _, IntoElement as _, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, div, px,
};

use super::preview::{WIDTH, preview};
use super::{Snap, widgets};
use crate::appearance::{Appearance, Listed};
use crate::notify::tip;
use crate::panels::settings::SettingsPanel;
use crate::theme::colours;
use crate::ui::{self, Icon};

fn badges(listed: &Listed, snap: &Snap) -> Vec<AnyElement> {
    if !snap.follows {
        return Vec::new();
    }
    let mut out = Vec::new();
    if snap.slots.0 == listed.id {
        out.push(ui::icon(Icon::Sun, 12.).text_color(colours::working()).into_any_element());
    }
    if snap.slots.1 == listed.id {
        out.push(ui::icon(Icon::Moon, 12.).text_color(colours::accent()).into_any_element());
    }
    out
}

fn caption(listed: &Listed, snap: &Snap) -> Div {
    let problem = listed.problem.clone().map(|why| {
        div()
            .id(SharedString::from(format!("problem-{}", listed.id)))
            .tooltip(tip(why))
            .child(ui::dot(Some(colours::failed())))
    });
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .text_size(px(12.))
        .child(div().min_w_0().flex().child(ui::one_line(listed.name.clone())))
        .children((!listed.preset).then(|| ui::quiet_chip("yours")))
        .children(problem)
        .children(badges(listed, snap))
}

impl SettingsPanel {
    fn theme_card(listed: &Listed, snap: &Snap, cx: &mut Context<Self>) -> AnyElement {
        let chosen = listed.id == snap.id;
        let id = listed.id.clone();
        let theme = Appearance::get(cx).read(cx).theme_of(&listed.id);
        let shown = match theme {
            Some(theme) => preview(&theme),
            None => div().w(px(WIDTH)).h(px(96.)).rounded(px(8.)).bg(colours::inset()),
        };
        let ring = if chosen { colours::brand() } else { gpui::transparent_black().into() };
        div()
            .id(SharedString::from(format!("theme-{}", listed.id)))
            .w(px(WIDTH + 8.))
            .p(px(3.))
            .flex()
            .flex_col()
            .gap(px(6.))
            .rounded(px(12.))
            .border_2()
            .border_color(ring)
            .cursor_pointer()
            .child(shown)
            .child(caption(listed, snap))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.close_picker(cx);
                Appearance::get(cx).update(cx, |appearance, cx| appearance.use_theme(&id, cx));
            }))
            .into_any_element()
    }

    pub(super) fn theme_group(snap: &Snap, cx: &mut Context<Self>) -> Div {
        let follow = widgets::row(
            true,
            widgets::label("Follow the system's light and dark", "Use one theme in light and another in dark."),
            ui::icon(Icon::SunMoon, 16.).text_color(colours::muted()),
            widgets::switch("appearance-follow", snap.follows).on_click(cx.listener(|_, _, _, cx| {
                Appearance::get(cx).update(cx, |appearance, cx| {
                    let on = !appearance.follows();
                    appearance.follow_system(on, cx);
                });
            })),
        );
        let cards: Vec<AnyElement> = snap.listed.iter().map(|listed| Self::theme_card(listed, snap, cx)).collect();
        let mut cards = cards.into_iter();
        let mut lines = Vec::new();
        loop {
            let line: Vec<AnyElement> = cards.by_ref().take(4).collect();
            if line.is_empty() {
                break;
            }
            lines.push(div().flex().gap(px(10.)).children(line));
        }
        let gallery = div().flex().flex_col().gap(px(10.)).py(px(12.)).children(lines);
        widgets::group("Theme", widgets::card().child(follow).child(gallery))
    }
}
