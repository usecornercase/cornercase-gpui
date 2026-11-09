use gpui::{
    AnyElement, Context, Corner, Div, Focusable as _, KeyDownEvent, MouseButton, MouseDownEvent, SharedString,
    Stateful, Window, anchored, deferred, div, point, prelude::*, px,
};

use super::data::{self, Choice, Setting};
use super::{OpenChoice, Role, SettingsPanel, widgets};
use crate::theme::colours;
use crate::ui::menu::{self, Inks};
use crate::ui::motion::{self, Way};
use crate::ui::{self, Icon};

const DROP: f32 = 4.;
const WIDTH: f32 = 340.;

fn note_of(choice: &Choice) -> Option<String> {
    (!choice.note.is_empty() && choice.note != choice.value).then(|| choice.note.clone())
}

fn choice_body(choice: &Choice, label: String, inks: &Inks) -> Div {
    let note = note_of(choice).map(|note| div().text_size(px(11.5)).text_color(inks.note).child(note));
    let line = div()
        .flex()
        .items_center()
        .gap(px(6.))
        .text_color(inks.value)
        .child(label)
        .children(choice.dangerous.then(|| ui::icon(Icon::Alert, 12.).text_color(inks.alert)));
    div().flex_1().min_w_0().flex().flex_col().gap(px(2.)).child(line).children(note)
}

fn choice_row(
    setting: &Setting,
    choice: &Choice,
    index: usize,
    lit: bool,
    cx: &mut Context<SettingsPanel>,
) -> AnyElement {
    let (key, value) = (setting.key.clone(), choice.value.clone());
    let inks = menu::inks(lit, choice.dangerous);
    let check = (choice.value == setting.text()).then(|| ui::icon(Icon::Check, 13.).text_color(inks.mark));
    menu::entry(SharedString::from(format!("option-{key}-{index}")), lit, check)
        .child(choice_body(choice, setting.offered_label(choice), &inks))
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
        let button = Self::opening(button, key, busy, cx);
        div().relative().flex().min_w_0().child(button).when(open, |d| d.child(self.choice_popover(setting, cx)))
    }

    fn opening(button: Stateful<Div>, key: String, busy: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        if busy {
            return button.opacity(0.5).cursor_default();
        }
        button.on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                cx.stop_propagation();
                this.toggle_choice(&key, window, cx);
            }),
        )
    }

    pub(super) fn suggestions_opener(&self, role: &Role, cx: &mut Context<Self>) -> Option<AnyElement> {
        let Role::Setting(key) = role else { return None };
        let busy = self.busy.contains(role);
        let button = ui::field_action(
            SharedString::from(format!("suggest-{key}")),
            Icon::ChevronDown,
            "Suggestions (↓)",
            colours::muted(),
        );
        Some(Self::opening(button, key.clone(), busy, cx).into_any_element())
    }

    pub(super) fn suggested(&self, setting: &Setting, field: Div, cx: &mut Context<Self>) -> Div {
        let open = self.choice.as_ref().is_some_and(|c| c.key == setting.key);
        div().relative().flex().min_w_0().child(field).when(open, |d| d.child(self.choice_popover(setting, cx)))
    }

    pub(super) fn suggestions_down(&mut self, role: &Role, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Role::Setting(key) = role else { return false };
        let suggests = self.setting(key).is_some_and(Setting::suggests);
        if !suggests || self.busy.contains(role) || self.choice.is_some() {
            return false;
        }
        let at = self.setting(key).and_then(Setting::in_use).or(Some(0));
        self.open_choice(key, at, window, cx);
        true
    }

    fn open_choice(&mut self, key: &str, at: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        self.highlight(key.to_string(), at);
        window.focus(&self.choice_focus);
        cx.notify();
    }

    fn highlight(&mut self, key: String, at: Option<usize>) {
        let above = menu::ABOVE + usize::from(self.setting(&key).is_some_and(|s| s.question.is_some()));
        if let Some(at) = at {
            self.choice_scroll.scroll_to_item(at + above);
        }
        self.choice = Some(OpenChoice { key, highlight: at });
    }

    fn refocus(&self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        if !self.setting(key).is_some_and(Setting::suggests) {
            return;
        }
        if let Some(field) = self.fields.get(&Role::Setting(key.to_string())) {
            window.focus(&field.read(cx).focus_handle(cx));
        }
    }

    pub(super) fn take_suggestion(&mut self, key: &str, value: &str, window: &mut Window, cx: &mut Context<Self>) {
        let role = Role::Setting(key.to_string());
        let Some(field) = self.fields.get(&role).cloned() else { return };
        field.update(cx, |f, cx| f.set_text(value.to_string(), cx));
        window.focus(&field.read(cx).focus_handle(cx));
        self.submit(&role, cx);
    }

    fn toggle_choice(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        let open = self.choice.as_ref().is_some_and(|c| c.key == key);
        if open {
            self.choice = None;
            self.refocus(key, window, cx);
            cx.notify();
        } else {
            self.open_choice(key, None, window, cx);
        }
    }

    fn hover_choice(&mut self, index: usize, hovered: bool, cx: &mut Context<Self>) {
        let Some(open) = self.choice.as_mut() else { return };
        open.highlight = menu::hovered(open.highlight, index, hovered);
        cx.notify();
    }

    pub(super) fn close_choice(&mut self, cx: &mut Context<Self>) {
        if self.choice.take().is_some() {
            cx.notify();
        }
    }

    fn choice_popover(&self, setting: &Setting, cx: &mut Context<Self>) -> Div {
        let highlight = self.choice.as_ref().and_then(|c| c.highlight);
        let rows: Vec<AnyElement> = setting
            .offered()
            .iter()
            .enumerate()
            .map(|(i, choice)| choice_row(setting, choice, i, highlight == Some(i), cx))
            .collect();
        let question = setting
            .question
            .clone()
            .map(|q| div().px(px(10.)).pt(px(6.)).pb(px(5.)).text_size(px(11.5)).text_color(colours::muted()).child(q));
        let card = menu::card("settings-choices", &self.choice_focus, &self.choice_scroll)
            .w(px(WIDTH))
            .max_h(px(380.))
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
        let Some(values) = self.setting(&key).map(|s| s.offered().into_iter().map(|c| c.value).collect::<Vec<_>>())
        else {
            return;
        };
        match event.keystroke.key.as_str() {
            "escape" => {
                self.choice = None;
                self.refocus(&key, window, cx);
            }
            "down" | "up" => {
                let moved = menu::step(highlight, values.len(), event.keystroke.key == "down");
                self.highlight(key, moved);
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
