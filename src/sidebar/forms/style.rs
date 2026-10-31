use gpui::{
    App, AppContext as _, ClickEvent, Context, Div, FocusHandle, Focusable, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _, px,
};
use serde_json::Value;

use super::{Check, Checked, close, keyed, load_check, palette_colour, show, warning};
use crate::dialogs::frame;
use crate::store::{Store, args};
use crate::theme::{colours, radius};
use crate::ui;

pub(in crate::sidebar) struct GroupStyle {
    group: u64,
    name: String,
    check: Check,
    focus: FocusHandle,
}

pub(in crate::sidebar) fn group_style(group: u64, name: String, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| {
        load_check(args(&["group", "style", &group.to_string(), "--json"]), cx);
        GroupStyle { group, name, check: Check::Loading, focus: cx.focus_handle() }
    });
    show(view, window, cx);
}

struct Choices {
    icon: String,
    colour: u8,
    icons: Vec<String>,
    colours: Vec<u8>,
}

fn array<'a>(v: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    v.get(key).and_then(Value::as_array).into_iter().flatten()
}

fn palette_index(v: &Value) -> Option<u8> {
    v.as_u64().and_then(|c| u8::try_from(c).ok())
}

impl Choices {
    fn of(v: &Value) -> Self {
        Choices {
            icon: v.get("icon").and_then(Value::as_str).unwrap_or_default().to_string(),
            colour: v.get("colour").and_then(palette_index).unwrap_or(4),
            icons: array(v, "icons").filter_map(Value::as_str).map(String::from).collect(),
            colours: array(v, "colours").filter_map(palette_index).collect(),
        }
    }
}

impl GroupStyle {
    fn set(&mut self, key: &str, value: Value, cx: &mut Context<Self>) {
        let text = match &value {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        let flag = if key == "icon" { "--icon" } else { "--colour" };
        Store::get(cx).read(cx).cmd(args(&["group", "style", &self.group.to_string(), flag, &text]));
        if let Check::Ready(v) = &mut self.check
            && let Some(obj) = v.as_object_mut()
        {
            obj.insert(key.to_string(), value);
        }
        cx.notify();
    }

    fn icon_choice(i: usize, glyph: String, choices: &Choices, cx: &mut Context<Self>) -> Stateful<Div> {
        let selected = glyph == choices.icon;
        div()
            .id(SharedString::from(format!("icon-{i}")))
            .size(px(30.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(radius::CONTROL))
            .border_1()
            .border_color(if selected { colours::accent() } else { colours::border() })
            .when(selected, |d| d.bg(colours::selected()))
            .text_color(palette_colour(choices.colour))
            .cursor_pointer()
            .hover(|s| s.bg(colours::hover()))
            .child(glyph.clone())
            .on_click(
                cx.listener(move |this, _: &ClickEvent, _, cx| this.set("icon", Value::String(glyph.clone()), cx)),
            )
    }

    fn colour_choice(c: u8, selected: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id(SharedString::from(format!("colour-{c}")))
            .size(px(22.))
            .rounded_full()
            .border_2()
            .border_color(if selected { colours::text() } else { gpui::transparent_black().into() })
            .bg(palette_colour(c))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set("colour", Value::from(c), cx)))
    }

    fn choices(&self, choices: Choices, cx: &mut Context<Self>) -> Div {
        let icons: Vec<_> =
            choices.icons.iter().enumerate().map(|(i, g)| Self::icon_choice(i, g.clone(), &choices, cx)).collect();
        let swatches: Vec<_> =
            choices.colours.iter().map(|&c| Self::colour_choice(c, c == choices.colour, cx)).collect();
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(div().text_size(px(18.)).text_color(palette_colour(choices.colour)).child(choices.icon))
                    .child(ui::title(self.name.clone())),
            )
            .child(ui::field("Icon", div().flex().flex_wrap().gap(px(4.)).children(icons)))
            .child(ui::field("Colour", div().flex().flex_wrap().gap(px(6.)).children(swatches)))
    }
}

impl Checked for GroupStyle {
    fn check(&mut self) -> &mut Check {
        &mut self.check
    }
}

impl Focusable for GroupStyle {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for GroupStyle {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match &self.check {
            Check::Loading => ui::busy("style-loading", "Loading…"),
            Check::Failed(e) => warning(e.clone()),
            Check::Ready(v) => self.choices(Choices::of(v), cx),
        };
        keyed(&self.focus, |_: &mut Self, cx| close(cx), cx).child(frame(
            "Icon and colour",
            body,
            ui::primary_button("done", "Done").on_click(|_: &ClickEvent, _, cx| close(cx)),
        ))
    }
}
