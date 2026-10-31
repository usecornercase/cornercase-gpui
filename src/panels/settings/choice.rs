use gpui::{
    AnyElement, Context, Corner, Div, KeyDownEvent, MouseButton, MouseDownEvent, SharedString, Window, anchored,
    deferred, div, point, prelude::*, px,
};

use super::data::{self, Choice, Setting};
use super::{OpenChoice, Role, SettingsPanel, widgets};
use crate::theme::{colours, radius};
use crate::ui::motion::{self, Way};
use crate::ui::{self, Icon, glass};

const DROP: f32 = 4.;
const WIDTH: f32 = 340.;

fn step(highlight: Option<usize>, count: usize, down: bool) -> Option<usize> {
    let last = count.checked_sub(1)?;
    Some(match (highlight, down) {
        (None, true) => 0,
        (None, false) => last,
        (Some(at), true) => (at + 1).min(last),
        (Some(at), false) => at.saturating_sub(1),
    })
}

fn note_of(choice: &Choice) -> Option<String> {
    (!choice.note.is_empty() && choice.note != choice.value).then(|| choice.note.clone())
}

struct Inks {
    value: gpui::Rgba,
    note: gpui::Rgba,
    mark: gpui::Rgba,
    alert: gpui::Rgba,
}

fn inks(lit: bool, dangerous: bool) -> Inks {
    if lit {
        let on = colours::on_brand();
        return Inks { value: on, note: glass::tint(on, 0.8), mark: on, alert: on };
    }
    let value = if dangerous { colours::failed() } else { colours::text() };
    Inks { value, note: colours::faint(), mark: colours::accent(), alert: colours::failed() }
}

fn choice_body(choice: &Choice, inks: &Inks) -> Div {
    let note = note_of(choice).map(|note| div().text_size(px(11.5)).text_color(inks.note).child(note));
    let line = div()
        .flex()
        .items_center()
        .gap(px(6.))
        .text_color(inks.value)
        .child(data::spoken(&choice.value))
        .children(choice.dangerous.then(|| ui::icon(Icon::Alert, 12.).text_color(inks.alert)));
    div().flex_1().min_w_0().flex().flex_col().gap(px(2.)).child(line).children(note)
}

fn choice_row(setting: &Setting, index: usize, lit: bool, cx: &mut Context<SettingsPanel>) -> AnyElement {
    let choice = &setting.choices[index];
    let (key, value) = (setting.key.clone(), choice.value.clone());
    let inks = inks(lit, choice.dangerous);
    let check = (choice.value == setting.text()).then(|| ui::icon(Icon::Check, 13.).text_color(inks.mark));
    div()
        .id(SharedString::from(format!("option-{key}-{index}")))
        .flex()
        .items_start()
        .gap(px(8.))
        .px(px(10.))
        .py(px(6.))
        .rounded(px(radius::CONTROL))
        .cursor_pointer()
        .when(lit, |d| d.bg(glass::cobalt()))
        .child(div().w(px(13.)).pt(px(2.)).flex_none().children(check))
        .child(choice_body(choice, &inks))
        .on_hover(cx.listener(move |this, hovered: &bool, _, cx| this.hover_choice(index, *hovered, cx)))
        .on_click(cx.listener(move |this, _, window, cx| this.choose(key.clone(), value.clone(), window, cx)))
        .into_any_element()
}

impl SettingsPanel {
    pub(super) fn choice_button(&self, setting: &Setting, cx: &mut Context<Self>) -> Div {
        let key = setting.key.clone();
        let open = self.choice.as_ref().is_some_and(|c| c.key == key);
        let busy = self.busy.contains(&Role::Setting(key.clone()));
        let id = SharedString::from(format!("pick-{key}"));
        let button = widgets::dropdown(id, data::spoken(&setting.text()), setting.risky_now(), open).flex_1();
        let button = if busy {
            button.opacity(0.5).cursor_default()
        } else {
            button.on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.toggle_choice(&key, window, cx);
                }),
            )
        };
        div().relative().flex().min_w_0().child(button).when(open, |d| d.child(self.choice_popover(setting, cx)))
    }

    fn toggle_choice(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        let open = self.choice.as_ref().is_some_and(|c| c.key == key);
        self.choice = (!open).then(|| OpenChoice { key: key.to_string(), highlight: None });
        if !open {
            window.focus(&self.choice_focus);
        }
        cx.notify();
    }

    fn hover_choice(&mut self, index: usize, hovered: bool, cx: &mut Context<Self>) {
        let Some(open) = self.choice.as_mut() else { return };
        if hovered {
            open.highlight = Some(index);
        } else if open.highlight == Some(index) {
            open.highlight = None;
        }
        cx.notify();
    }

    pub(super) fn close_choice(&mut self, cx: &mut Context<Self>) {
        if self.choice.take().is_some() {
            cx.notify();
        }
    }

    fn choice_popover(&self, setting: &Setting, cx: &mut Context<Self>) -> Div {
        let highlight = self.choice.as_ref().and_then(|c| c.highlight);
        let rows: Vec<AnyElement> =
            (0..setting.choices.len()).map(|i| choice_row(setting, i, highlight == Some(i), cx)).collect();
        let question = setting
            .question
            .clone()
            .map(|q| div().px(px(10.)).pt(px(6.)).pb(px(5.)).text_size(px(11.5)).text_color(colours::muted()).child(q));
        let card = glass::card(radius::CARD)
            .id("settings-choices")
            .track_focus(&self.choice_focus)
            .track_scroll(&self.choice_scroll)
            .occlude()
            .w(px(WIDTH))
            .max_h(px(380.))
            .overflow_y_scroll()
            .p(px(4.))
            .flex()
            .flex_col()
            .gap(px(1.))
            .text_size(px(12.5))
            .on_mouse_down(MouseButton::Left, |_: &MouseDownEvent, _, cx| cx.stop_propagation())
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| this.choice_key(e, window, cx)))
            .children(question)
            .children(rows);
        let id = SharedString::from(format!("settings-choices-in-{}", setting.key));
        let placed =
            anchored().anchor(Corner::TopRight).offset(point(px(0.), px(6.))).snap_to_window_with_margin(px(8.));
        div()
            .absolute()
            .top_full()
            .right_0()
            .child(deferred(placed.child(motion::slide(card, id, Way::Pop, (0., -DROP)))).with_priority(1))
    }

    fn choice_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(open) = self.choice.as_ref() else { return };
        let (key, highlight) = (open.key.clone(), open.highlight);
        let Some(values) = self.setting(&key).map(|s| s.choices.iter().map(|c| c.value.clone()).collect::<Vec<_>>())
        else {
            return;
        };
        match event.keystroke.key.as_str() {
            "escape" => self.choice = None,
            "down" | "up" => {
                let moved = step(highlight, values.len(), event.keystroke.key == "down");
                let above = 1 + usize::from(self.setting(&key).is_some_and(|s| s.question.is_some()));
                if let Some(at) = moved {
                    self.choice_scroll.scroll_to_item(at + above);
                }
                self.choice = Some(OpenChoice { key, highlight: moved });
            }
            "enter" => {
                if let Some(value) = highlight.and_then(|i| values.get(i)).cloned() {
                    self.choose(key, value, window, cx);
                }
            }
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }
}

#[cfg(test)]
mod tests;
