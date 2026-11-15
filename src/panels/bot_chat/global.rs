use gpui::{AnyView, App, AppContext as _, Entity, Global, StyleRefinement, Styled as _};

use super::{BotChat, Scroll};
use crate::store::Store;

struct GlobalBotChat(Entity<BotChat>);
impl Global for GlobalBotChat {}

pub(crate) fn view(cx: &mut App) -> AnyView {
    AnyView::from(BotChat::get(cx)).cached(StyleRefinement::default().size_full())
}

pub(crate) fn scroll(step: Scroll, cx: &mut App) {
    BotChat::get(cx).update(cx, |chat, cx| chat.scroll(step, cx));
}

impl BotChat {
    pub(crate) fn get(cx: &mut App) -> Entity<Self> {
        if let Some(GlobalBotChat(chat)) = cx.try_global::<GlobalBotChat>() {
            return chat.clone();
        }
        let store = Store::get(cx);
        let chat = cx.new(|cx| Self::new(store, cx));
        cx.set_global(GlobalBotChat(chat.clone()));
        chat
    }
}
