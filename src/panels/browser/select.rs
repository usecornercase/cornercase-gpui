use gpui::{
    AnyElement, ClickEvent, Context, InteractiveElement as _, IntoElement, Keystroke, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, anchored, deferred, div, point, prelude::FluentBuilder as _, px,
};
use serde_json::{Value, json};

use super::Browser;
use super::cdp::Event;
use super::viewport::{Rect, from_page};
use crate::context_menu::menu_row;
use crate::theme::{colours, radius};
use crate::ui::glass;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Item {
    pub(crate) label: String,
    pub(crate) disabled: bool,
    pub(crate) group: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Choice {
    pub(crate) items: Vec<Item>,
    pub(crate) selected: Option<usize>,
    pub(crate) highlight: Option<usize>,
    pub(crate) rect: Rect,
    pub(crate) session: Option<String>,
    pub(crate) context: Option<i64>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Row<'a> {
    Group(&'a str),
    Item(usize),
}

fn item(value: &Value) -> Item {
    Item {
        label: value["label"].as_str().unwrap_or_default().to_string(),
        disabled: value["disabled"].as_bool().unwrap_or(false),
        group: value["group"].as_str().unwrap_or_default().to_string(),
    }
}

impl Choice {
    pub(crate) fn read(message: &Value, event: &Event) -> Option<Self> {
        let items: Vec<Item> = message["options"].as_array()?.iter().map(item).collect();
        let selected = message["selected"].as_u64().and_then(|i| usize::try_from(i).ok()).filter(|&i| i < items.len());
        Some(Self {
            items,
            selected,
            highlight: selected,
            rect: Rect::read(message)?,
            session: event.session.clone(),
            context: event.params["executionContextId"].as_i64(),
        })
    }

    fn choosable(&self, at: usize) -> bool {
        self.items.get(at).is_some_and(|i| !i.disabled)
    }

    pub(crate) fn step(&mut self, down: bool) {
        let len = self.items.len();
        let next = |i: usize| if down { Some(i + 1).filter(|&n| n < len) } else { i.checked_sub(1) };
        let mut at = match self.highlight {
            Some(i) => next(i),
            None if down => Some(0),
            None => len.checked_sub(1),
        };
        while let Some(i) = at.filter(|&i| !self.choosable(i)) {
            at = next(i);
        }
        if at.is_some() {
            self.highlight = at;
        }
    }

    pub(crate) fn seek(&mut self, typed: char) {
        let len = self.items.len();
        let from = self.highlight.map_or(0, |i| i + 1);
        let starts = |i: usize| {
            let label = self.items[i].label.trim_start().chars().next();
            self.choosable(i) && label.is_some_and(|c| c.to_lowercase().eq(typed.to_lowercase()))
        };
        if let Some(found) = (0..len).map(|n| (from + n) % len).find(|&i| starts(i)) {
            self.highlight = Some(found);
        }
    }

    pub(crate) fn rows(&self) -> Vec<Row<'_>> {
        let mut rows = Vec::new();
        let mut group = "";
        for (i, item) in self.items.iter().enumerate() {
            if item.group != group && !item.group.is_empty() {
                rows.push(Row::Group(&item.group));
            }
            group = &item.group;
            rows.push(Row::Item(i));
        }
        rows
    }
}

pub(crate) fn choose_script(index: Option<usize>, context: Option<i64>) -> Value {
    let index = index.map_or(-1, |i| i64::try_from(i).unwrap_or(-1));
    let mut params = json!({ "expression": format!("window.__cornercase && window.__cornercase.choose({index})") });
    if let Some(context) = context {
        params["contextId"] = json!(context);
    }
    params
}

impl Browser {
    pub(super) fn open_choice(&mut self, message: &Value, event: &Event, cx: &mut Context<Self>) {
        self.input.choice = Choice::read(message, event);
        cx.notify();
    }

    fn settle_choice(&mut self, index: Option<usize>, cx: &mut Context<Self>) {
        let Some(choice) = self.input.choice.take() else { return };
        let index = index.filter(|&i| choice.choosable(i));
        if let Some(session) = choice.session.as_deref() {
            self.send(Some(session), "Runtime.evaluate", choose_script(index, choice.context));
        }
        cx.notify();
    }

    pub(super) fn close_choice(&mut self, cx: &mut Context<Self>) {
        self.settle_choice(None, cx);
    }

    pub(super) fn choice_key(&mut self, key: &Keystroke, cx: &mut Context<Self>) {
        let Some(choice) = self.input.choice.as_mut() else { return };
        match key.key.as_str() {
            "up" | "down" => choice.step(key.key == "down"),
            "enter" | "space" => {
                let highlight = choice.highlight;
                self.settle_choice(highlight, cx);
            }
            "tab" => self.close_choice(cx),
            _ => {
                if let Some(typed) = key.key_char.as_deref().and_then(|t| t.chars().next()) {
                    choice.seek(typed);
                }
            }
        }
        cx.notify();
    }

    pub(super) fn choice_menu(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let choice = self.input.choice.as_ref().filter(|c| c.session == self.active_session())?;
        let meta = self.tabs.active(self.workspace)?.shot.as_ref()?.meta;
        let at = from_page(choice.rect, self.input.bounds.get(), &meta);
        let rows: Vec<AnyElement> = choice.rows().into_iter().map(|row| choice_row(choice, &row, cx)).collect();
        let list = glass::card(radius::CARD)
            .id("browser-select")
            .occlude()
            .min_w(at.size.width.max(px(160.)))
            .max_w(px(420.))
            .max_h(px(320.))
            .overflow_y_scroll()
            .p(px(4.))
            .flex()
            .flex_col()
            .text_size(px(12.))
            .children(rows)
            .on_mouse_down_out(cx.listener(|this, _, _, cx| this.close_choice(cx)));
        let below = point(at.origin.x, at.origin.y + at.size.height + px(2.));
        Some(deferred(anchored().position(below).snap_to_window_with_margin(px(8.)).child(list)).with_priority(2))
    }
}

fn choice_row(choice: &Choice, row: &Row<'_>, cx: &mut Context<Browser>) -> AnyElement {
    let i = match row {
        Row::Group(label) => return group_label((*label).to_string()).into_any_element(),
        Row::Item(i) => *i,
    };
    let item = &choice.items[i];
    let label = SharedString::from(item.label.clone());
    if item.disabled {
        return div().px(px(10.)).py(px(5.)).text_color(colours::faint()).child(label).into_any_element();
    }
    let id = SharedString::from(format!("browser-option-{i}"));
    menu_row(id, label, false, (choice.highlight == Some(i), false))
        .when(choice.selected == Some(i), |row| row.child(div().text_color(colours::accent()).child("✓")))
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.settle_choice(Some(i), cx)))
        .into_any_element()
}

fn group_label(label: String) -> gpui::Div {
    div().px(px(10.)).pt(px(6.)).pb(px(2.)).text_size(px(11.)).text_color(colours::faint()).child(label)
}

#[cfg(test)]
mod tests;
