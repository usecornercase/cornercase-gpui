use gpui::{
    App, ClickEvent, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, div, prelude::FluentBuilder as _, px,
};

use crate::dialogs;
use crate::keymap::{Command, keys_of};
use crate::model::bots::{IDEAS, face_of_name};
use crate::platform::Os;
use crate::store::Store;
use crate::theme::{colours, radius};
use crate::ui::{self, Icon, Tone};

const FACE: f32 = 58.;
const WIDTH: f32 = 470.;
const IDEA_FACE: f32 = 26.;
const IDEA_TEXT: f32 = 50.;

pub(crate) fn lobby(cx: &App) -> impl IntoElement + use<> {
    let lobby = Store::get(cx).read(cx).bots_lobby();
    let page = div()
        .w(px(WIDTH))
        .max_w_full()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(12.))
        .child(ui::face::crowd(&IDEAS.map(|(name, _)| face_of_name(name)), FACE).mb(px(8.)))
        .child(div().text_size(px(20.)).font_weight(FontWeight::SEMIBOLD).text_center().child(lobby.heading()))
        .children(lobby.lines().into_iter().map(|line| ui::emblem_line(line).text_size(px(13.))))
        .when(lobby.makes(), |d| d.child(new_bot_button()))
        .when(lobby.ideas(), |d| d.child(ideas()));
    div()
        .id("bots-lobby")
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .overflow_y_scroll()
        .rounded(px(radius::card()))
        .bg(colours::pane())
        .border_1()
        .border_color(colours::border())
        .px(px(28.))
        .child(div().flex_1().min_h(px(40.)))
        .child(page)
        .child(div().flex_1().min_h(px(40.)))
}

fn new_bot_button() -> impl IntoElement {
    let keys = keys_of(Command::NewBot, Os::CURRENT).unwrap_or_default();
    ui::icon_text_button("bots-lobby-new", Icon::Plus, "New bot", Tone::Primary)
        .mt(px(8.))
        .px(px(16.))
        .h(px(32.))
        .when(!keys.is_empty(), |b| b.child(div().text_size(px(11.)).opacity(0.7).child(keys)))
        .on_click(|_: &ClickEvent, window, cx| dialogs::new_bot(window, cx))
}

fn ideas() -> Div {
    let cards = IDEAS.iter().enumerate().map(|(at, (name, about))| idea(at, name, about));
    div()
        .w_full()
        .mt(px(18.))
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(
            div()
                .text_size(px(11.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(colours::muted())
                .child("Start from an idea"),
        )
        .children(cards)
}

fn idea(at: usize, name: &'static str, about: &'static str) -> impl IntoElement {
    div()
        .id(("bots-lobby-idea", at))
        .relative()
        .flex()
        .flex_col()
        .pl(px(IDEA_TEXT))
        .pr(px(12.))
        .py(px(10.))
        .rounded(px(radius::card()))
        .bg(colours::panel())
        .border_1()
        .border_color(colours::border())
        .cursor_pointer()
        .hover(|s| s.bg(colours::hover()))
        .child(div().absolute().left(px(12.)).top(px(12.)).child(ui::face::face(face_of_name(name), IDEA_FACE)))
        .child(div().w_full().text_size(px(12.5)).font_weight(FontWeight::SEMIBOLD).child(name))
        .child(div().w_full().text_size(px(12.)).text_color(colours::muted()).child(about))
        .on_click(move |_: &ClickEvent, window, cx| dialogs::new_bot_for(name, about, window, cx))
}
