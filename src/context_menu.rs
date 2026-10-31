use std::rc::Rc;

use gpui::{
    AnyElement, ClickEvent, Context, Div, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, MouseButton,
    ParentElement, Pixels, Point, SharedString, Stateful, StatefulInteractiveElement, Styled, Window, anchored,
    deferred, div, prelude::FluentBuilder as _, px,
};

use crate::app::Root;
use crate::store::Busy;
use crate::theme::{colours, radius};
use crate::ui::glass;
use crate::ui::motion::{self, Way};

const MENU_DROP: f32 = 4.;

pub(crate) type Action = Rc<dyn Fn(&mut Root, &mut Window, &mut Context<Root>)>;

#[derive(Clone)]
pub(crate) enum Entry {
    Item { label: SharedString, action: Action, danger: bool },
    Note(SharedString),
    Submenu { label: SharedString, entries: Vec<Entry> },
    Separator,
}
pub(crate) fn item(
    label: impl Into<SharedString>,
    action: impl Fn(&mut Root, &mut Window, &mut Context<Root>) + 'static,
) -> Entry {
    Entry::Item { label: label.into(), action: Rc::new(action), danger: false }
}
pub(crate) fn danger(
    label: impl Into<SharedString>,
    action: impl Fn(&mut Root, &mut Window, &mut Context<Root>) + 'static,
) -> Entry {
    Entry::Item { label: label.into(), action: Rc::new(action), danger: true }
}
pub(crate) fn act(label: impl Into<SharedString>, args: Vec<String>) -> Entry {
    item(label, move |root, _, _| root.act(args.clone()))
}

pub(crate) fn act_busy(label: impl Into<SharedString>, busy: Busy, args: Vec<String>) -> Entry {
    item(label, move |root, _, cx| {
        let (busy, args) = (busy.clone(), args.clone());
        root.store.update(cx, |s, cx| s.act_busy(busy, args, cx));
    })
}

pub(crate) fn submenu(label: impl Into<SharedString>, entries: Vec<Entry>) -> Entry {
    Entry::Submenu { label: label.into(), entries }
}

pub(crate) struct Menu {
    pub at: Point<Pixels>,
    pub entries: Vec<Entry>,
    pub focus: FocusHandle,
    pub highlight: Option<usize>,
}

fn choosable(entry: &Entry) -> bool {
    matches!(entry, Entry::Item { .. } | Entry::Submenu { .. })
}

fn step(entries: &[Entry], from: Option<usize>, down: bool) -> Option<usize> {
    let choices: Vec<usize> = entries.iter().enumerate().filter(|(_, e)| choosable(e)).map(|(i, _)| i).collect();
    let at = from.and_then(|from| choices.iter().position(|&i| i == from));
    let next = match at {
        None if down => 0,
        None => choices.len().checked_sub(1)?,
        Some(at) if down => (at + 1) % choices.len(),
        Some(at) => at.checked_sub(1).unwrap_or(choices.len() - 1),
    };
    choices.get(next).copied()
}

fn without_stray_separators(entries: Vec<Entry>) -> Vec<Entry> {
    let mut clean: Vec<Entry> = Vec::new();
    for entry in entries {
        let separator = matches!(entry, Entry::Separator);
        if separator && clean.last().is_none_or(|e| matches!(e, Entry::Separator)) {
            continue;
        }
        clean.push(entry);
    }
    while clean.last().is_some_and(|e| matches!(e, Entry::Separator)) {
        clean.pop();
    }
    clean
}

impl Root {
    pub(crate) fn open_menu(
        &mut self,
        at: Point<Pixels>,
        entries: Vec<Entry>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let entries = without_stray_separators(entries);
        if entries.is_empty() {
            return;
        }
        let focus = cx.focus_handle();
        window.focus(&focus);
        self.side.menu = Some(Menu { at, entries, focus, highlight: None });
        cx.notify();
    }

    pub(crate) fn close_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.side.menu.take().is_some() {
            window.focus(&self.focus);
            cx.notify();
        }
    }

    fn open_submenu(&mut self, entries: &[Entry], cx: &mut Context<Self>) {
        if let Some(menu) = &mut self.side.menu {
            menu.entries = entries.to_vec();
            menu.highlight = None;
            cx.notify();
        }
    }

    fn menu_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        match key {
            "escape" => self.close_menu(window, cx),
            "down" | "up" => {
                if let Some(menu) = &mut self.side.menu {
                    menu.highlight = step(&menu.entries, menu.highlight, key == "down");
                    cx.notify();
                }
            }
            "enter" | "space" => self.choose_highlighted(window, cx),
            _ => {}
        }
    }

    fn choose_highlighted(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self.side.menu.as_ref().and_then(|m| m.entries.get(m.highlight?).cloned()) else { return };
        match entry {
            Entry::Item { action, .. } => {
                self.close_menu(window, cx);
                action(self, window, cx);
            }
            Entry::Submenu { entries, .. } => {
                self.open_submenu(&entries, cx);
                if let Some(menu) = &mut self.side.menu {
                    menu.highlight = step(&menu.entries, None, true);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn render_menu(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let menu = self.side.menu.as_ref()?;
        let list = menu_frame(&menu.focus, cx)
            .children(
                menu.entries
                    .iter()
                    .enumerate()
                    .map(|(i, entry)| menu_entry(i, entry.clone(), menu.highlight == Some(i), cx)),
            )
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
        let id = SharedString::from(format!("menu-{:?}", menu.at));
        let list = motion::slide(list, id, Way::Pop, (0., -MENU_DROP));
        Some(deferred(anchored().position(menu.at).snap_to_window_with_margin(px(8.)).child(list)).with_priority(2))
    }
}

fn menu_frame(focus: &FocusHandle, cx: &mut Context<Root>) -> Stateful<Div> {
    glass::card(radius::CARD)
        .id("context-menu")
        .track_focus(focus)
        .min_w(px(200.))
        .max_w(px(320.))
        .p(px(4.))
        .flex()
        .flex_col()
        .text_size(px(12.))
        .on_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
            if !e.keystroke.modifiers.platform {
                this.menu_key(e.keystroke.key.as_str(), window, cx);
                cx.stop_propagation();
            }
        }))
        .on_mouse_down_out(cx.listener(|this, _, window, cx| this.close_menu(window, cx)))
}

fn menu_entry(index: usize, entry: Entry, highlighted: bool, cx: &mut Context<Root>) -> AnyElement {
    let id = SharedString::from(format!("menu-{index}"));
    match entry {
        Entry::Separator => div().mx(px(10.)).my(px(4.)).h(px(1.)).bg(colours::border()).into_any_element(),
        Entry::Note(text) => div().px(px(10.)).py(px(5.)).text_color(colours::faint()).child(text).into_any_element(),
        Entry::Item { label, action, danger } => menu_row(id, label, false, (highlighted, danger))
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.close_menu(window, cx);
                action(this, window, cx);
            }))
            .into_any_element(),
        Entry::Submenu { label, entries } => menu_row(id, label, true, (highlighted, false))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.open_submenu(&entries, cx)))
            .into_any_element(),
    }
}

pub(crate) fn menu_row(
    id: SharedString,
    label: SharedString,
    more: bool,
    (highlighted, danger): (bool, bool),
) -> Stateful<Div> {
    let (lit, ink, lit_ink) = if danger {
        (glass::tint(colours::failed(), 0.16).into(), colours::failed(), colours::failed())
    } else {
        (glass::cobalt(), colours::text(), colours::on_brand())
    };
    div()
        .id(id)
        .px(px(10.))
        .py(px(5.))
        .rounded(px(radius::CONTROL))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(16.))
        .cursor_pointer()
        .text_color(ink)
        .when(highlighted, |d| d.bg(lit).text_color(lit_ink))
        .hover(|s| s.bg(lit).text_color(lit_ink))
        .child(div().whitespace_nowrap().child(label))
        .when(more, |d| d.child(div().text_color(colours::muted()).child("›")))
}

#[cfg(test)]
mod tests;
