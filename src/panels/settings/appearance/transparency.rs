use gpui::{
    AnyElement, Context, Div, IntoElement as _, ParentElement as _, SharedString, StatefulInteractiveElement as _,
    Styled as _, div, px,
};

use super::{Snap, widgets};
use crate::appearance::{Appearance, Part};
use crate::panels::settings::SettingsPanel;
use crate::platform::Os;
use crate::theme::colours;
use crate::theme::current::opacity_override;
use crate::theme::tokens::Opacity;
use crate::ui::{self, Icon, slider};

struct Line {
    key: &'static str,
    label: &'static str,
    note: &'static str,
}

const LINES: [Line; 5] = [
    Line {
        key: "ground",
        label: "Window glass",
        note: "How much of the window's ground shows over the desktop. Below 55 % the status bar's faint text got lost over a bright desktop.",
    },
    Line {
        key: "panes",
        label: "Terminal panes",
        note: "The terminal's background over the ground. A pane cannot be more see-through than the ground.",
    },
    Line {
        key: "cards",
        label: "Cards",
        note: "The cards of the sidebar and panels. At 0 they vanish; above 80 % they turn into opaque blocks.",
    },
    Line {
        key: "floating",
        label: "Floating cards",
        note: "Menus, dialogs and sheets. Kept nearly opaque: the window does not blur its own content, so text under a card would show sharp.",
    },
    Line { key: "veil", label: "Dimming", note: "What dims the window behind a sheet." },
];

fn percent(value: f32) -> String {
    format!("{:.0}%", value * 100.)
}

fn range_of(key: &str, snap: &Snap) -> (f32, f32) {
    let at = Opacity::KEYS.iter().position(|k| *k == key).unwrap_or(0);
    let (least, most) = Opacity::LIMITS[at];
    if key == "panes" && colours::glass().a < 1. {
        return (snap.theme.opacity.ground.max(least), most);
    }
    (least, most)
}

fn ground_note() -> Option<&'static str> {
    if colours::glass().a < 1. {
        return None;
    }
    Some(match Os::CURRENT {
        Os::Windows => "Windows refused its acrylic backdrop, so the ground is drawn opaque.",
        _ => "This system draws the ground opaque: compositors here do not blur what is behind a window.",
    })
}

impl SettingsPanel {
    fn opacity_row(first: bool, line: &Line, snap: &Snap, cx: &mut Context<Self>) -> AnyElement {
        let value = snap.theme.opacity.get(line.key).unwrap_or(1.);
        let label = widgets::label(line.label, line.note);
        let notice = match line.key {
            "ground" => ground_note().map(str::to_string),
            "panes" => opacity_override().map(|_| "Set by CORNERCASE_GPUI_OPACITY".to_string()),
            _ => None,
        };
        let control = match &notice {
            Some(note) => div().w(px(200.)).text_size(px(11.5)).text_color(colours::muted()).child(note.clone()),
            None => Self::slider_control(line.key, value, range_of(line.key, snap)),
        };
        widgets::row(first, label, control, Self::opacity_reset(line.key, value, snap, cx)).into_any_element()
    }

    fn slider_control(key: &'static str, value: f32, range: (f32, f32)) -> Div {
        let bar = slider::slider(key, value, range, 0.01, move |chosen, cx| {
            Appearance::get(cx).update(cx, |appearance, cx| appearance.set_opacity(key, Some(chosen), cx));
        });
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(bar)
            .child(div().w(px(38.)).text_size(px(11.5)).text_color(colours::muted()).child(percent(value)))
    }

    fn opacity_reset(key: &'static str, value: f32, snap: &Snap, cx: &mut Context<Self>) -> AnyElement {
        if !snap.changed(Part::Opacity, key, value) {
            return div().into_any_element();
        }
        let base = snap.base.as_ref().and_then(|base| base.opacity.get(key)).unwrap_or(value);
        ui::row_action(
            SharedString::from(format!("reset-opacity-{key}")),
            Icon::Undo,
            format!("Back to the base's: {}", percent(base)),
            false,
        )
        .on_click(cx.listener(move |_, _, _, cx| {
            Appearance::get(cx).update(cx, |appearance, cx| appearance.set_opacity(key, None, cx));
        }))
        .into_any_element()
    }

    pub(super) fn transparency_group(snap: &Snap, cx: &mut Context<Self>) -> Div {
        let rows: Vec<AnyElement> =
            LINES.iter().enumerate().map(|(i, line)| Self::opacity_row(i == 0, line, snap, cx)).collect();
        widgets::group("Transparency", widgets::card().children(rows))
    }
}
