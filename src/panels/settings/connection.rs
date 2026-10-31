use gpui::{
    AnyElement, Context, Div, Focusable as _, IntoElement, Rgba, SharedString, Stateful, Window, div, prelude::*, px,
};

use super::data::{self, Connection, ConnectionKind, Tone};
use super::pages::Slot;
use super::{Role, SettingsPanel, widgets};
use crate::theme::colours;
use crate::ui::{self, Icon};

const TOKEN: f32 = 230.;

fn tone_dot(tone: Tone) -> (Option<Rgba>, Rgba) {
    match tone {
        Tone::Good => (Some(colours::done()), colours::done()),
        Tone::Bad => (Some(colours::failed()), colours::failed()),
        Tone::Quiet => (None, colours::muted()),
    }
}

fn pairing_note(c: &Connection) -> Option<String> {
    let paired = c.state == data::State::Connected;
    if c.kind != ConnectionKind::Chat || paired {
        return None;
    }
    let what = c.note.strip_prefix("enter ").unwrap_or("shows a code to scan");
    Some(format!("Pair it from cornercase's own window, Settings › Remote: it {what}."))
}

fn connection_label(c: &Connection) -> Div {
    let (status, tone) = c.status();
    let (dot, ink) = tone_dot(tone);
    let note = if c.note.starts_with("enter ") { String::new() } else { widgets::capitalised(&c.note) };
    let title = div().flex().flex_col().gap(px(4.)).child(widgets::capitalised(&c.label)).child(widgets::status(
        widgets::capitalised(&status),
        dot,
        ink,
    ));
    widgets::labelled(title, note).children(pairing_note(c).map(|n| ui::dim(n).text_size(px(11.5))))
}

impl SettingsPanel {
    pub(super) fn render_connection(&self, c: &Connection, slot: Slot<'_>, cx: &mut Context<Self>) -> AnyElement {
        let role = Role::Secret(c.key.clone());
        let control = if c.takes_secret() { self.token_control(c, slot.window, cx) } else { div() };
        div()
            .flex()
            .flex_col()
            .child(widgets::row(slot.first, connection_label(c), control, self.connection_slot(c, cx)))
            .children(self.hint_of(&role, c.hint(), slot.window, cx))
            .children(self.error_of(&role))
            .into_any_element()
    }

    fn token_control(&self, c: &Connection, window: &Window, cx: &mut Context<Self>) -> Div {
        let Some(field) = self.fields.get(&Role::Secret(c.key.clone())) else { return div() };
        let focused = field.read(cx).focus_handle(cx).is_focused(window);
        let typed = !field.read(cx).text().trim().is_empty();
        let key_ink = if focused { colours::accent() } else { colours::faint() };
        let inner = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(ui::icon(Icon::Key, 13.).text_color(key_ink))
            .child(div().flex_1().min_w_0().child(field.clone()));
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(ui::input_box(inner, focused).w(px(TOKEN)).py(px(5.)).overflow_hidden())
            .child(self.connect_button(c, typed, cx))
    }

    fn connect_button(&self, c: &Connection, typed: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let key = c.key.clone();
        let id = SharedString::from(format!("connect-{key}"));
        if self.busy.contains(&Role::Secret(key.clone())) {
            return ui::busy_button(id, "Checking…", ui::Tone::Primary);
        }
        let label = if c.state == data::State::Missing { "Connect" } else { "Replace" };
        if !typed {
            return ui::resting_button(id, label, ui::Tone::Primary);
        }
        ui::primary_button(id, label).on_click(cx.listener(move |this, _, _, cx| this.connect(&key, cx)))
    }

    fn connection_slot(&self, c: &Connection, cx: &mut Context<Self>) -> AnyElement {
        let busy = self.busy.contains(&Role::Secret(c.key.clone()));
        if busy && !c.takes_secret() {
            let id = SharedString::from(format!("saving-secret-{}", c.key));
            return ui::spinner(id, 13., colours::muted()).into_any_element();
        }
        if !c.removable || busy {
            return div().into_any_element();
        }
        let hint = if c.kind == ConnectionKind::Browsers { "Forget every paired browser…" } else { "Remove…" };
        let connection = c.clone();
        ui::row_action(SharedString::from(format!("remove-{}", c.key)), Icon::Trash, hint, true)
            .on_click(cx.listener(move |_, _, window, cx| Self::ask_remove(&connection, window, cx)))
            .into_any_element()
    }
}
