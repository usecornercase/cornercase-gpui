use gpui::{
    AnyElement, AnyView, App, AppContext as _, Entity, FocusHandle, Global, IntoElement as _, ParentElement as _,
    StyleRefinement, Styled as _, div, px,
};

use super::field::ChatField;
use super::{BotChat, Scroll};
use crate::store::Store;
use crate::theme::{colours, radius};

struct GlobalBotChat(Entity<BotChat>);
impl Global for GlobalBotChat {}

struct GlobalChatField(Entity<ChatField>);
impl Global for GlobalChatField {}

pub(crate) fn view(cx: &mut App) -> AnyElement {
    let lines = AnyView::from(BotChat::get(cx)).cached(StyleRefinement::default().flex_1().min_h_0().w_full());
    let field = ChatField::get(cx);
    div()
        .size_full()
        .flex()
        .flex_col()
        .overflow_hidden()
        .rounded(px(radius::card()))
        .bg(colours::pane())
        .border_1()
        .border_color(colours::border())
        .child(lines)
        .child(field)
        .into_any_element()
}

pub(crate) fn scroll(step: Scroll, cx: &mut App) {
    BotChat::get(cx).update(cx, |chat, cx| chat.scroll(step, cx));
}

pub(super) fn to_end(cx: &mut App) {
    BotChat::get(cx).update(cx, BotChat::scroll_to_end);
}

pub(crate) fn field_focus(cx: &mut App) -> FocusHandle {
    let field = ChatField::get(cx);
    field.read(cx).focus_handle(cx)
}

impl BotChat {
    pub(crate) fn get(cx: &mut App) -> Entity<Self> {
        let store = Store::get(cx);
        if let Some(GlobalBotChat(chat)) = cx.try_global::<GlobalBotChat>()
            && chat.read(cx).store == store
        {
            return chat.clone();
        }
        let chat = cx.new(|cx| Self::new(store, cx));
        cx.set_global(GlobalBotChat(chat.clone()));
        chat
    }
}

impl ChatField {
    pub(super) fn get(cx: &mut App) -> Entity<Self> {
        let store = Store::get(cx);
        if let Some(GlobalChatField(field)) = cx.try_global::<GlobalChatField>()
            && field.read(cx).store == store
        {
            return field.clone();
        }
        let field = cx.new(|cx| Self::new(store, cx));
        cx.set_global(GlobalChatField(field.clone()));
        field
    }
}
