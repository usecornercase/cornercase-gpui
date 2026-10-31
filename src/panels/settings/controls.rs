use gpui::{
    AnyElement, Context, Div, Focusable as _, IntoElement, SharedString, Stateful, Window, div, prelude::*, px,
};

use super::data::{self, Kind, Setting};
use super::{Role, SettingsPanel, role_id, widgets};
use crate::notify::tip;
use crate::theme::{FONT_FAMILY, colours};
use crate::ui::{self, Icon};

#[derive(Clone, Copy)]
pub(super) struct Look {
    pub width: f32,
    pub mono: bool,
}

impl Look {
    pub(super) const SCRIPT: Look = Look { width: 300., mono: true };
    pub(super) const ARGUMENTS: Look = Look { width: 230., mono: true };

    fn of(kind: Kind) -> Self {
        let width = match kind {
            Kind::Number => 84.,
            Kind::Key => 170.,
            _ => 300.,
        };
        Self { width, mono: false }
    }
}

fn with_note(segment: Stateful<Div>, note: &str, value: &str) -> Stateful<Div> {
    if note.is_empty() || note == value {
        return segment;
    }
    segment.tooltip(tip(note.to_string()))
}

fn sources_summary(setting: &Setting) -> Div {
    let (shown, all) = (setting.items().len(), data::sources(setting).len());
    div().text_size(px(12.)).text_color(colours::muted()).child(format!("{shown} of {all} shown"))
}

impl SettingsPanel {
    pub(super) fn render_control(&self, setting: &Setting, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let busy = self.busy.contains(&Role::Setting(setting.key.clone()));
        match setting.kind {
            Kind::Switch => Self::switch(setting, busy, cx).into_any_element(),
            Kind::Choice if setting.as_segments() => Self::segments(setting, busy, cx).into_any_element(),
            Kind::Choice => self.choice_button(setting, cx).into_any_element(),
            Kind::List => sources_summary(setting).into_any_element(),
            Kind::Text | Kind::Number | Kind::Key | Kind::Other => self.text_setting(setting, window, cx),
        }
    }

    fn text_setting(&self, setting: &Setting, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let role = Role::Setting(setting.key.clone());
        let field = self.render_text(&role, Look::of(setting.kind), window, cx);
        let Some(unit) = setting.unit() else { return field.into_any_element() };
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(field)
            .child(div().text_size(px(12.)).text_color(colours::muted()).child(unit))
            .into_any_element()
    }

    fn switch(setting: &Setting, busy: bool, cx: &mut Context<Self>) -> Div {
        let (on, key) = (setting.on(), setting.key.clone());
        let switch = widgets::switch(SharedString::from(format!("switch-{key}")), on);
        let switch = if busy {
            switch.opacity(0.5).cursor_default()
        } else {
            switch.on_click(cx.listener(move |this, _, _, cx| this.save(&key, &(!on).to_string(), cx)))
        };
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(div().text_size(px(12.)).text_color(colours::muted()).child(setting.switch_text().to_string()))
            .child(switch)
    }

    fn segments(setting: &Setting, busy: bool, cx: &mut Context<Self>) -> Div {
        let current = setting.text();
        let items = setting
            .choices
            .iter()
            .map(|choice| {
                let (key, value) = (setting.key.clone(), choice.value.clone());
                let id = SharedString::from(format!("choice-{key}-{value}"));
                let segment = with_note(ui::segment(id, data::spoken(&value), value == current), &choice.note, &value);
                if busy {
                    return segment.cursor_default().into_any_element();
                }
                segment
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.choose(key.clone(), value.clone(), window, cx)),
                    )
                    .into_any_element()
            })
            .collect();
        ui::segments(items).when(busy, |d| d.opacity(0.5))
    }

    pub(super) fn render_text(&self, role: &Role, look: Look, window: &Window, cx: &mut Context<Self>) -> Div {
        let Some(field) = self.fields.get(role) else { return div() };
        let focused = field.read(cx).focus_handle(cx).is_focused(window);
        let tail = self.field_tail(role, cx);
        let inner = div().flex().items_center().gap(px(4.)).child(div().flex_1().min_w_0().child(field.clone()));
        ui::input_box(inner.children(tail), focused)
            .w(px(look.width))
            .py(px(5.))
            .overflow_hidden()
            .when(look.mono, |d| d.font_family(FONT_FAMILY).text_size(px(12.)))
    }

    fn field_tail(&self, role: &Role, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.busy.contains(role) {
            let id = SharedString::from(format!("saving-{}", role_id(role)));
            return Some(ui::spinner(id, 12., colours::muted()).into_any_element());
        }
        if self.edited(role, cx) {
            return Some(Self::edit_actions(role, cx).into_any_element());
        }
        let saved = self.saved.as_ref() == Some(role);
        saved.then(|| ui::icon(Icon::Check, 12.).text_color(colours::done()).into_any_element())
    }

    fn edit_actions(role: &Role, cx: &mut Context<Self>) -> Div {
        let id = role_id(role);
        let (save, revert) = (role.clone(), role.clone());
        div()
            .flex()
            .items_center()
            .gap(px(2.))
            .child(
                ui::field_action(
                    SharedString::from(format!("revert-{id}")),
                    Icon::Close,
                    "Revert (Escape)",
                    colours::muted(),
                )
                .on_click(cx.listener(move |this, _, _, cx| this.revert(&revert, cx))),
            )
            .child(
                ui::field_action(
                    SharedString::from(format!("save-{id}")),
                    Icon::Check,
                    "Save (Enter)",
                    colours::accent(),
                )
                .on_click(cx.listener(move |this, _, _, cx| this.submit(&save, cx))),
            )
    }
}
