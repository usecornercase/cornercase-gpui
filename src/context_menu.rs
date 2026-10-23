//! The menu a right-click or a row's `⋯` opens: entries that act on the window, a submenu that
//! takes the menu's place (as the TUI's **move to group** does), drawn over everything at the
//! pointer. `Escape`, a click outside or any entry closes it.

use std::rc::Rc;

use gpui::{
    ClickEvent, Context, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, MouseButton, ParentElement,
    Pixels, Point, SharedString, StatefulInteractiveElement, Styled, Window, anchored, deferred, div,
    prelude::FluentBuilder as _, px,
};

use crate::app::Root;
use crate::theme::colours;

/// What an entry does, run on the window once the menu has closed.
pub type Action = Rc<dyn Fn(&mut Root, &mut Window, &mut Context<Root>)>;

#[derive(Clone)]
pub enum Entry {
    Item { label: SharedString, action: Action, danger: bool },
    /// Grey text that does nothing: why something is missing.
    Note(SharedString),
    Submenu { label: SharedString, entries: Vec<Entry> },
    Separator,
}

/// An entry that runs `action`.
pub fn item(label: impl Into<SharedString>, action: impl Fn(&mut Root, &mut Window, &mut Context<Root>) + 'static) -> Entry {
    Entry::Item { label: label.into(), action: Rc::new(action), danger: false }
}

/// An entry for something that stops or removes things, in red.
pub fn danger(label: impl Into<SharedString>, action: impl Fn(&mut Root, &mut Window, &mut Context<Root>) + 'static) -> Entry {
    Entry::Item { label: label.into(), action: Rc::new(action), danger: true }
}

/// An entry that runs a command in our view (`Root::act`).
pub fn act(label: impl Into<SharedString>, args: Vec<String>) -> Entry {
    item(label, move |root, _, _| root.act(args.clone()))
}

pub fn submenu(label: impl Into<SharedString>, entries: Vec<Entry>) -> Entry {
    Entry::Submenu { label: label.into(), entries }
}

pub struct Menu {
    pub at: Point<Pixels>,
    pub entries: Vec<Entry>,
    pub focus: FocusHandle,
}

impl Root {
    pub(crate) fn open_menu(&mut self, at: Point<Pixels>, entries: Vec<Entry>, window: &mut Window, cx: &mut Context<Self>) {
        // Separators at either end or twice in a row come from optional entries left out.
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
        if clean.is_empty() {
            return;
        }
        let focus = cx.focus_handle();
        window.focus(&focus);
        self.side.menu = Some(Menu { at, entries: clean, focus });
        cx.notify();
    }

    pub(crate) fn close_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.side.menu.take().is_some() {
            window.focus(&self.focus);
            cx.notify();
        }
    }

    /// The open menu, drawn over everything; `None` while none is open.
    pub(crate) fn render_menu(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let menu = self.side.menu.as_ref()?;
        let mut list = div()
            .id("context-menu")
            .track_focus(&menu.focus)
            .min_w(px(190.))
            .max_w(px(320.))
            .py(px(4.))
            .flex()
            .flex_col()
            .rounded(px(7.))
            .bg(colours::panel())
            .border_1()
            .border_color(colours::border())
            .shadow_lg()
            .text_size(px(12.))
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                if e.keystroke.key == "escape" {
                    this.close_menu(window, cx);
                }
                cx.stop_propagation();
            }))
            .on_mouse_down_out(cx.listener(|this, _, window, cx| this.close_menu(window, cx)));
        for (i, entry) in menu.entries.iter().enumerate() {
            let id = SharedString::from(format!("menu-{i}"));
            list = list.child(match entry.clone() {
                Entry::Separator => div().my(px(4.)).h(px(1.)).bg(colours::border()).into_any_element(),
                Entry::Note(text) => {
                    div().px(px(12.)).py(px(4.)).text_color(colours::faint()).child(text).into_any_element()
                }
                Entry::Item { label, action, danger } => menu_row(id, label, false)
                    .when(danger, |d| d.text_color(colours::failed()))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.close_menu(window, cx);
                        action(this, window, cx);
                    }))
                    .into_any_element(),
                Entry::Submenu { label, entries } => menu_row(id, label, true)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(menu) = &mut this.side.menu {
                            menu.entries = entries.clone();
                            cx.notify();
                        }
                    }))
                    .into_any_element(),
            });
        }
        // Clicks inside must not reach what lies under the menu.
        let list = list.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
        Some(deferred(anchored().position(menu.at).snap_to_window_with_margin(px(8.)).child(list)).with_priority(2))
    }
}

fn menu_row(id: SharedString, label: SharedString, more: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .mx(px(4.))
        .px(px(8.))
        .py(px(4.))
        .rounded(px(4.))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(16.))
        .cursor_pointer()
        .text_color(colours::text())
        .hover(|s| s.bg(colours::selected()))
        .child(div().whitespace_nowrap().child(label))
        .when(more, |d| d.child(div().text_color(colours::faint()).child("›")))
}
