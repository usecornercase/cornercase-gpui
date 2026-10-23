use gpui::{AnyElement, Context, Div, Focusable as _, IntoElement, SharedString, Window, div, prelude::*, px};

use super::data::{self, Connection, ConnectionKind, Tone};
use super::pages::Slot;
use super::{Pending, Role, SettingsPanel, widgets};
use crate::theme::colours;
use crate::ui;

fn tone_colour(tone: Tone) -> gpui::Rgba {
    match tone {
        Tone::Good => colours::done(),
        Tone::Bad => colours::failed(),
        Tone::Quiet => colours::faint(),
    }
}

fn faint_note(text: impl Into<SharedString>) -> Div {
    div().text_size(px(11.5)).text_color(colours::faint()).child(text.into())
}

fn connection_label(c: &Connection) -> Div {
    let (status, tone) = c.status();
    let note = if c.note.starts_with("enter ") { String::new() } else { c.note.clone() };
    let pairs_elsewhere = c.kind == ConnectionKind::Chat && c.state != data::State::Connected;
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(
            div()
                .flex()
                .gap(px(8.))
                .child(widgets::capitalised(&c.label))
                .child(div().text_size(px(12.)).text_color(tone_colour(tone)).child(status)),
        )
        .when(!note.is_empty(), |d| d.child(faint_note(note)))
        .when(pairs_elsewhere, |d| {
            d.child(faint_note("pairing shows a code to scan: do it from cornercase's own window, Settings › Remote"))
        })
}

fn remove_question(c: &Connection) -> String {
    match c.kind {
        ConnectionKind::Browsers => "Forget every paired browser? They need pairing again.".to_string(),
        _ => format!("Remove {}? It has to be set again to use it.", c.label),
    }
}

impl SettingsPanel {
    pub(super) fn render_connection(&self, c: &Connection, slot: Slot<'_>, cx: &mut Context<Self>) -> AnyElement {
        let role = Role::Secret(c.key.clone());
        let control = if self.busy.contains(&role) {
            div().flex().items_center().gap(px(6.)).child(ui::dim(if c.takes_secret() {
                "checking…"
            } else {
                "saving…"
            }))
        } else {
            self.connection_controls(c, slot.window, cx)
        };
        let confirm = (self.pending == Some(Pending::Remove(c.key.clone()))).then(|| {
            let key = c.key.clone();
            let go = ui::danger_button("pending-go", "Remove")
                .on_click(cx.listener(move |this, _, _, cx| this.remove(&key, cx)));
            Self::ask(remove_question(c), go, cx)
        });
        div()
            .flex()
            .flex_col()
            .child(widgets::row(slot.first, connection_label(c), control))
            .children(confirm)
            .children(self.error_of(&role))
            .into_any_element()
    }

    fn token_controls(&self, c: &Connection, window: &Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(field) = self.fields.get(&Role::Secret(c.key.clone())).filter(|_| c.takes_secret()) else {
            return Vec::new();
        };
        let focused = field.entity.read(cx).focus_handle(cx).is_focused(window);
        let mut controls =
            vec![ui::input_box(field.entity.clone(), focused).w(px(240.)).overflow_hidden().into_any_element()];
        if field.entity.read(cx).text().trim().is_empty() {
            return controls;
        }
        let key = c.key.clone();
        let connect = if c.state == data::State::Missing { "Connect" } else { "Replace" };
        controls.push(
            ui::primary_button(SharedString::from(format!("connect-{key}")), connect)
                .on_click(cx.listener(move |this, _, _, cx| this.connect(&key, cx)))
                .into_any_element(),
        );
        controls
    }

    fn connection_controls(&self, c: &Connection, window: &Window, cx: &mut Context<Self>) -> Div {
        let key = c.key.clone();
        let token = self.token_controls(c, window, cx);
        let remove = c.removable.then(|| {
            let what = if c.kind == ConnectionKind::Browsers { "Forget" } else { "Remove" };
            ui::button(SharedString::from(format!("remove-{key}")), what).on_click(cx.listener(
                move |this, _, _, cx| {
                    this.pending = Some(Pending::Remove(key.clone()));
                    cx.notify();
                },
            ))
        });
        div().flex().items_center().gap(px(6.)).children(token).children(remove)
    }
}
