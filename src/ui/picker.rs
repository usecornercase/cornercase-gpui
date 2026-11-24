use gpui::{
    Context, Corner, Div, EventEmitter, FocusHandle, InteractiveElement as _, IntoElement, KeyDownEvent, MouseButton,
    MouseDownEvent, ParentElement as _, Render, ScrollHandle, SharedString, Stateful, StatefulInteractiveElement as _,
    Styled as _, Window, anchored, deferred, div, point, prelude::FluentBuilder as _, px,
};

use super::motion::{self, Way};
use super::{Icon, glass, menu, text_field};
pub(crate) use crate::model::accounts::pick::PickRow;
use crate::theme::colours;

pub(crate) trait Pickable: 'static {
    fn key(&self) -> String;
    fn rows(&self) -> Vec<PickRow>;
    fn shown(&self) -> PickRow;
    fn gone(&self) -> Option<String>;
    fn choose(&mut self, folder: &str);
    fn chosen_at(&self) -> Option<usize>;
}

const WIDTH: f32 = 300.;
const MENU_HEIGHT: f32 = 268.;
const BELOW: f32 = 6.;
const DROP: f32 = 4.;

pub(crate) struct Picked;

pub(crate) struct Picker<P> {
    pub(crate) picking: P,
    open: bool,
    highlight: Option<usize>,
    focus: FocusHandle,
    scroll: ScrollHandle,
    before: Option<FocusHandle>,
    scroller: Option<ScrollHandle>,
    was_focused: bool,
}

fn reveal_first(scroller: &ScrollHandle, bounds: &[gpui::Bounds<gpui::Pixels>], window: &Window) {
    if let Some(first) = bounds.first() {
        text_field::show_box(scroller, (first.top(), first.size.height), window);
    }
}

pub(crate) fn reveals(focused: bool, was_focused: bool, open: bool) -> bool {
    focused && !was_focused && !open
}

impl<P: Pickable> EventEmitter<Picked> for Picker<P> {}

impl<P: Pickable> Picker<P> {
    pub(crate) fn new(picking: P, cx: &mut Context<Self>) -> Self {
        Self {
            picking,
            open: false,
            highlight: None,
            focus: cx.focus_handle().tab_stop(true),
            scroll: ScrollHandle::new(),
            before: None,
            scroller: None,
            was_focused: false,
        }
    }

    pub(crate) fn follow(&mut self, change: impl FnOnce(&mut P) -> bool, cx: &mut Context<Self>) {
        if change(&mut self.picking) {
            cx.notify();
        }
    }

    pub(crate) fn inside(mut self, scroller: Option<ScrollHandle>) -> Self {
        self.scroller = scroller;
        self
    }

    fn id(&self, what: &str) -> SharedString {
        SharedString::from(format!("{}-{what}", self.picking.key()))
    }

    fn show_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.before = window.focused(cx).filter(|focused| *focused != self.focus);
        self.open = true;
        self.highlight = self.picking.chosen_at();
        window.focus(&self.focus);
        cx.notify();
    }

    fn close_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !std::mem::take(&mut self.open) {
            return;
        }
        if let Some(before) = self.before.take() {
            window.focus(&before);
        }
        cx.notify();
    }

    fn choose(&mut self, folder: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.picking.choose(folder);
        cx.emit(Picked);
        self.close_menu(window, cx);
    }

    fn choose_highlighted(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rows = self.picking.rows();
        match self.highlight.and_then(|at| rows.get(at)) {
            Some(row) => self.choose(&row.folder, window, cx),
            None => self.close_menu(window, cx),
        }
    }

    fn step(&mut self, down: bool, cx: &mut Context<Self>) {
        self.highlight = menu::step(self.highlight, self.picking.rows().len(), down);
        if let Some(at) = self.highlight {
            self.scroll.scroll_to_item(at + menu::ABOVE);
        }
        cx.notify();
    }

    fn menu_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        match key {
            "escape" | "tab" => self.close_menu(window, cx),
            "down" | "up" => self.step(key == "down", cx),
            "enter" | "space" => self.choose_highlighted(window, cx),
            _ => return false,
        }
        true
    }

    fn closed_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) -> bool {
        match event.keystroke.key.as_str() {
            "down" | "up" | "enter" | "space" => self.show_menu(window, cx),
            "tab" if event.keystroke.modifiers.shift => window.focus_prev(),
            "tab" => window.focus_next(),
            _ => return false,
        }
        true
    }

    fn key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let taken = if self.open {
            self.menu_key(&event.keystroke.key, window, cx)
        } else {
            self.closed_key(event, window, cx)
        };
        if taken {
            cx.stop_propagation();
        }
    }

    fn trigger(&self, window: &Window, cx: &mut Context<Self>) -> Stateful<Div> {
        let shown = self.picking.shown();
        let gone = self.picking.gone().is_some();
        let focused = !self.open && self.focus.is_focused(window);
        let name = div().flex().min_w_0().text_color(if gone { colours::failed() } else { colours::text() });
        menu::dropdown(self.id("picker"), menu::edge(self.open || focused, gone))
            .track_focus(&self.focus)
            .max_w(px(WIDTH))
            .py(px(6.))
            .text_size(px(13.))
            .when(focused, |d| d.shadow(glass::ring(colours::brand())))
            .child(name.child(super::one_line(shown.name)))
            .child(div().flex_1().min_w_0().flex().text_color(colours::faint()).child(super::one_line(shown.note)))
            .child(menu::chevron())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.show_menu(window, cx);
                }),
            )
    }

    fn revealing(trigger: Stateful<Div>, scroller: Option<ScrollHandle>) -> Div {
        div().child(trigger).when_some(scroller, |d, scroller| {
            d.on_children_prepainted(move |bounds, window, _| reveal_first(&scroller, &bounds, window))
        })
    }

    fn row(&self, index: usize, row: PickRow, cx: &mut Context<Self>) -> Stateful<Div> {
        let lit = self.highlight == Some(index);
        let inks = menu::inks(lit, false);
        let check = row.chosen.then(|| super::icon(Icon::Check, 13.).text_color(inks.mark));
        let line = div()
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(div().flex().min_w_0().text_color(inks.value).child(super::one_line(row.name)))
            .child(div().flex_1().min_w_0().flex().text_color(inks.note).child(super::one_line(row.note)));
        let folder = row.folder;
        menu::entry((self.id("choice"), index), lit, check)
            .child(line)
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                this.highlight = menu::hovered(this.highlight, index, *hovered);
                cx.notify();
            }))
            .on_click(cx.listener(move |this, _, window, cx| this.choose(&folder, window, cx)))
    }

    fn popover(&self, cx: &mut Context<Self>) -> impl IntoElement + use<P> {
        let rows: Vec<Stateful<Div>> =
            self.picking.rows().into_iter().enumerate().map(|(index, row)| self.row(index, row, cx)).collect();
        let card = menu::card(self.id("choices"), &self.focus, &self.scroll)
            .w(px(WIDTH))
            .max_h(px(MENU_HEIGHT))
            .children(rows);
        let placed =
            anchored().anchor(Corner::TopLeft).offset(point(px(0.), px(BELOW))).snap_to_window_with_margin(px(8.));
        let shown = motion::slide(card, self.id("choices-in"), Way::Pop, (0., -DROP));
        div().absolute().top_full().left_0().child(deferred(placed.child(shown)).with_priority(2))
    }
}

impl<P: Pickable> Render for Picker<P> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.open && !self.focus.is_focused(window) {
            self.open = false;
            self.before = None;
        }
        let close = cx.listener(|this, _: &MouseDownEvent, window, cx| this.close_menu(window, cx));
        let focused = self.focus.is_focused(window);
        let reveal = self.scroller.clone().filter(|_| reveals(focused, self.was_focused, self.open));
        self.was_focused = focused;
        let control = div()
            .relative()
            .flex()
            .flex_col()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| this.key(event, window, cx)))
            .child(Self::revealing(self.trigger(window, cx), reveal))
            .when(self.open, |d| d.child(menu::backdrop(self.id("backdrop"), window, close)).child(self.popover(cx)));
        div().flex().flex_col().gap(px(6.)).child(control).children(self.picking.gone().map(super::error_line))
    }
}

#[cfg(test)]
mod tests;
