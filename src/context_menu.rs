use std::rc::Rc;

use gpui::{
    AnyElement, ClickEvent, Context, Div, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, MouseButton,
    ParentElement, Pixels, Point, SharedString, Stateful, StatefulInteractiveElement, Styled, Window, anchored,
    deferred, div, prelude::FluentBuilder as _, px,
};

use crate::app::Root;
use crate::theme::colours;
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

pub(crate) fn submenu(label: impl Into<SharedString>, entries: Vec<Entry>) -> Entry {
    Entry::Submenu { label: label.into(), entries }
}

pub(crate) struct Menu {
    pub at: Point<Pixels>,
    pub entries: Vec<Entry>,
    pub focus: FocusHandle,
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
        self.side.menu = Some(Menu { at, entries, focus });
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
            cx.notify();
        }
    }

    pub(crate) fn render_menu(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let menu = self.side.menu.as_ref()?;
        let list = menu_frame(&menu.focus, cx)
            .children(menu.entries.iter().enumerate().map(|(i, entry)| menu_entry(i, entry.clone(), cx)))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
        Some(deferred(anchored().position(menu.at).snap_to_window_with_margin(px(8.)).child(list)).with_priority(2))
    }
}

fn menu_frame(focus: &FocusHandle, cx: &mut Context<Root>) -> Stateful<Div> {
    div()
        .id("context-menu")
        .track_focus(focus)
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
        .on_mouse_down_out(cx.listener(|this, _, window, cx| this.close_menu(window, cx)))
}

fn menu_entry(index: usize, entry: Entry, cx: &mut Context<Root>) -> AnyElement {
    let id = SharedString::from(format!("menu-{index}"));
    match entry {
        Entry::Separator => div().my(px(4.)).h(px(1.)).bg(colours::border()).into_any_element(),
        Entry::Note(text) => div().px(px(12.)).py(px(4.)).text_color(colours::faint()).child(text).into_any_element(),
        Entry::Item { label, action, danger } => menu_row(id, label, false)
            .when(danger, |d| d.text_color(colours::failed()))
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.close_menu(window, cx);
                action(this, window, cx);
            }))
            .into_any_element(),
        Entry::Submenu { label, entries } => menu_row(id, label, true)
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.open_submenu(&entries, cx)))
            .into_any_element(),
    }
}

fn menu_row(id: SharedString, label: SharedString, more: bool) -> Stateful<Div> {
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
