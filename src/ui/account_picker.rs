use std::collections::BTreeMap;

use gpui::{
    App, AppContext as _, Context, Corner, Div, Entity, EventEmitter, FocusHandle, InteractiveElement as _,
    IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, ParentElement as _, Render, ScrollHandle, SharedString,
    Stateful, StatefulInteractiveElement as _, Styled as _, Window, anchored, deferred, div, point,
    prelude::FluentBuilder as _, px,
};

use super::motion::{self, Way};
use super::{Icon, glass, menu};
use crate::model::accounts::pick::{AccountPick, AgentChoices, PickRow, Picking};
use crate::theme::colours;

const WIDTH: f32 = 300.;
const MENU_HEIGHT: f32 = 268.;
const BELOW: f32 = 6.;
const DROP: f32 = 4.;

struct AccountPicked;

struct AccountPicker {
    picking: Picking,
    open: bool,
    highlight: Option<usize>,
    focus: FocusHandle,
    scroll: ScrollHandle,
    before: Option<FocusHandle>,
}

impl EventEmitter<AccountPicked> for AccountPicker {}

impl AccountPicker {
    fn new(picking: Picking, cx: &mut Context<Self>) -> Self {
        Self {
            picking,
            open: false,
            highlight: None,
            focus: cx.focus_handle().tab_stop(true),
            scroll: ScrollHandle::new(),
            before: None,
        }
    }

    fn follow(&mut self, choices: AgentChoices, cx: &mut Context<Self>) {
        if self.picking.follow(choices) {
            cx.notify();
        }
    }

    fn id(&self, what: &str) -> SharedString {
        SharedString::from(format!("account-{what}-{}", self.picking.agent()))
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
        cx.emit(AccountPicked);
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

    fn popover(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
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

impl Render for AccountPicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.open && !self.focus.is_focused(window) {
            self.open = false;
            self.before = None;
        }
        let close = cx.listener(|this, _: &MouseDownEvent, window, cx| this.close_menu(window, cx));
        let control = div()
            .relative()
            .flex()
            .flex_col()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| this.key(event, window, cx)))
            .child(self.trigger(window, cx))
            .when(self.open, |d| d.child(menu::backdrop(self.id("backdrop"), window, close)).child(self.popover(cx)));
        div().flex().flex_col().gap(px(6.)).child(control).children(self.picking.gone().map(super::error_line))
    }
}

#[derive(Default)]
pub(crate) struct AccountPickers {
    saved: BTreeMap<String, String>,
    pickers: Vec<Entity<AccountPicker>>,
}

impl AccountPickers {
    pub(crate) fn keeping(saved: BTreeMap<String, String>) -> Self {
        Self { saved, pickers: Vec::new() }
    }

    pub(crate) fn follow<T: 'static>(
        &mut self,
        list: Vec<AgentChoices>,
        on_pick: fn(&mut T, &mut Context<T>),
        cx: &mut Context<T>,
    ) {
        let old = std::mem::take(&mut self.pickers);
        self.pickers = list.into_iter().map(|choices| self.picker_for(choices, &old, on_pick, cx)).collect();
    }

    fn picker_for<T: 'static>(
        &self,
        choices: AgentChoices,
        old: &[Entity<AccountPicker>],
        on_pick: fn(&mut T, &mut Context<T>),
        cx: &mut Context<T>,
    ) -> Entity<AccountPicker> {
        if let Some(picker) = old.iter().find(|picker| picker.read(cx).picking.agent() == choices.agent) {
            picker.update(cx, |picker, cx| picker.follow(choices, cx));
            return picker.clone();
        }
        let saved = self.saved.get(&choices.agent).map(String::as_str);
        let picking = Picking::new(choices, saved);
        let picker = cx.new(|cx| AccountPicker::new(picking, cx));
        cx.subscribe(&picker, move |this, _, _: &AccountPicked, cx| on_pick(this, cx)).detach();
        picker
    }

    fn of<'a>(&'a self, agents: &'a [String], cx: &'a App) -> impl Iterator<Item = &'a Entity<AccountPicker>> {
        self.pickers.iter().filter(move |picker| agents.iter().any(|agent| agent == picker.read(cx).picking.agent()))
    }

    pub(crate) fn picks(&self, agents: &[String], cx: &App) -> Vec<AccountPick> {
        self.of(agents, cx).filter_map(|picker| picker.read(cx).picking.pick()).collect()
    }

    pub(crate) fn follow_in_use(&self, agents: &[String], cx: &App) -> bool {
        self.of(agents, cx).all(|picker| picker.read(cx).picking.follows_in_use())
    }

    pub(crate) fn fields(&self, agents: &[String], cx: &App) -> Option<Div> {
        let fields: Vec<Div> = self
            .of(agents, cx)
            .map(|picker| {
                let label = format!("{} account", picker.read(cx).picking.title());
                div().flex_1().min_w(px(200.)).child(super::field(label, picker.clone()))
            })
            .collect();
        (!fields.is_empty()).then(|| div().flex().flex_wrap().gap(px(10.)).children(fields))
    }
}
