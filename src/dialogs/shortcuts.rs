use gpui::{
    App, AppContext as _, ClickEvent, Context, Div, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use super::{close, frame};
use crate::keymap::{self, Group};
use crate::platform::Os;
use crate::store::Store;
use crate::theme::{FONT_FAMILY, colours, radius};
use crate::ui;

pub(crate) struct Shortcuts {
    focus: FocusHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Keys(Group),
    System,
    Elsewhere,
}

pub(crate) fn shortcuts(window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| Shortcuts { focus: cx.focus_handle() });
    let focus = view.read(cx).focus.clone();
    Store::get(cx).update(cx, |s, cx| s.show_dialog(view.into(), cx));
    window.focus(&focus);
}

fn columns(os: Os) -> [&'static [Section]; 3] {
    use Section::{Elsewhere, Keys, System};
    match os {
        Os::Mac => [
            &[Keys(Group::Panels), System],
            &[Keys(Group::Window), Keys(Group::GettingAround), Keys(Group::Terminal), Keys(Group::Viewer)],
            &[Keys(Group::TabsAndPanes), Keys(Group::Browser), Elsewhere],
        ],
        Os::Linux | Os::Windows => [
            &[Keys(Group::Panels), System],
            &[Keys(Group::TabsAndPanes), Keys(Group::GettingAround), Keys(Group::Terminal), Keys(Group::Viewer)],
            &[Keys(Group::Window), Keys(Group::Browser), Elsewhere],
        ],
    }
}

fn width(os: Os, window: f32) -> f32 {
    let wanted: f32 = match os {
        Os::Mac => 940.,
        Os::Linux | Os::Windows => 1120.,
    };
    wanted.min(window - 64.).max(480.)
}

fn body_height(window: f32) -> f32 {
    (window - 160.).max(200.)
}

fn keys(label: impl Into<SharedString>) -> Div {
    div()
        .flex_none()
        .px(px(6.))
        .py(px(1.))
        .rounded(px(radius::chip()))
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
        .py(px(1.5))
        .child(div().flex_1().min_w_0().text_color(colours::muted()).child(what))
        .child(keys(label))
}

fn block(title: &'static str, lines: impl IntoIterator<Item = Div>) -> Div {
    div().flex().flex_col().child(ui::section(title)).children(lines)
}

fn listed(title: &'static str, entries: &'static [(&'static str, &'static str)]) -> Div {
    block(title, entries.iter().map(|&(label, what)| line(label, what)))
}

fn section(section: Section, os: Os) -> Div {
    match section {
        Section::Keys(group) => {
            block(group.title(), keymap::in_group(group).map(|s| line(s.label(os), s.command.describe(os))))
        }
        Section::System => listed("System", keymap::system_keys(os)),
        Section::Elsewhere => listed("Everywhere else", keymap::other_keys(os)),
    }
}

fn column(sections: &[Section], os: Os) -> Div {
    div().flex_1().min_w_0().flex().flex_col().children(sections.iter().map(|&s| section(s, os)))
}

fn body(os: Os, height: f32) -> impl IntoElement {
    div()
        .id("shortcuts-body")
        .max_h(px(height))
        .overflow_y_scroll()
        .flex()
        .items_start()
        .gap(px(24.))
        .text_size(px(12.))
        .children(columns(os).map(|sections| column(sections, os)))
}

impl Render for Shortcuts {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let (os, size) = (Os::CURRENT, window.viewport_size());
        div().track_focus(&self.focus).child(
            frame(
                "Keyboard shortcuts",
                body(os, body_height(f32::from(size.height))),
                ui::primary_button("shortcuts-close", "Done").on_click(|_: &ClickEvent, _, cx| close(cx)),
            )
            .w(px(width(os, f32::from(size.width)))),
        )
    }
}

#[cfg(test)]
mod tests;
