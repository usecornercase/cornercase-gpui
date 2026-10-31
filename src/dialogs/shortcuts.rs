use gpui::{
    App, AppContext as _, ClickEvent, Context, Div, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use super::{close, frame};
use crate::keymap::{self, Group, OTHER_KEYS, SYSTEM_KEYS};
use crate::store::Store;
use crate::theme::{FONT_FAMILY, colours, radius};
use crate::ui;

pub(crate) struct Shortcuts {
    focus: FocusHandle,
}

pub(crate) fn shortcuts(window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| Shortcuts { focus: cx.focus_handle() });
    let focus = view.read(cx).focus.clone();
    Store::get(cx).update(cx, |s, cx| s.show_dialog(view.into(), cx));
    window.focus(&focus);
}

fn keys(label: impl Into<SharedString>) -> Div {
    div()
        .flex_none()
        .px(px(6.))
        .py(px(1.))
        .rounded(px(radius::CHIP))
        .bg(colours::selected())
        .font_family(FONT_FAMILY)
        .text_size(px(11.))
        .text_color(colours::text())
        .child(label.into())
}

fn line(label: impl Into<SharedString>, what: &'static str) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.))
        .py(px(2.))
        .child(div().flex_1().min_w_0().text_color(colours::muted()).child(ui::one_line(what)))
        .child(keys(label))
}

fn block(title: &'static str, lines: impl IntoIterator<Item = Div>) -> Div {
    div().flex().flex_col().child(ui::section(title)).children(lines)
}

fn group(group: Group) -> Div {
    block(group.title(), keymap::in_group(group).map(|s| line(s.label(), s.command.describe())))
}

fn listed(title: &'static str, entries: &'static [(&'static str, &'static str)]) -> Div {
    block(title, entries.iter().map(|&(label, what)| line(label, what)))
}

fn column() -> Div {
    div().flex_1().min_w_0().flex().flex_col()
}

fn body() -> impl IntoElement {
    div()
        .id("shortcuts-body")
        .max_h(px(480.))
        .overflow_y_scroll()
        .flex()
        .gap(px(24.))
        .text_size(px(12.))
        .child(column().child(group(Group::Panels)).child(listed("System", SYSTEM_KEYS)))
        .child(column().child(group(Group::Window)).child(group(Group::Terminal)))
        .child(
            column()
                .child(group(Group::TabsAndPanes))
                .child(group(Group::Browser))
                .child(listed("Everywhere else", OTHER_KEYS)),
        )
}

impl Render for Shortcuts {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().track_focus(&self.focus).child(
            frame(
                "Keyboard shortcuts",
                body(),
                ui::primary_button("shortcuts-close", "Done").on_click(|_: &ClickEvent, _, cx| close(cx)),
            )
            .w(px(940.)),
        )
    }
}
