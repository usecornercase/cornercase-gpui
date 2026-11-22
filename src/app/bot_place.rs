use gpui::{Context, Entity, Window};

use super::Root;
use crate::dialogs;
use crate::model::bots::{Here, keys_of, opens, sendable};
use crate::panels::bot_chat;
use crate::store::Store;

impl Root {
    pub(super) fn follow_place(&mut self, store: &Entity<Store>, window: &mut Window, cx: &mut Context<Self>) {
        let (place, bot, page) = {
            let s = store.read(cx);
            (s.machine_place().cloned(), s.shown_bot().cloned(), s.shows_bots())
        };
        let was = self.side.bots;
        let moved = bot != self.side.bot || page != self.side.bots;
        let rested = self.keys_rest(cx);
        self.side.bot = bot;
        self.side.bots = page;
        if place != self.side.machine_place {
            self.machine_place_changed(place, window, cx);
        }
        if moved {
            self.chat_place_changed(was, window, cx);
            self.keys_follow(&rested, window, cx);
        }
        self.end_gone_rename(window, cx);
    }

    fn chat_place_changed(&mut self, was: bool, window: &Window, cx: &mut Context<Self>) {
        match (was, self.side.bots) {
            (false, true) => {
                self.drop_drags();
                self.own.pause_surface();
            }
            (true, false) => self.back_to_own(cx),
            _ => {}
        }
        self.activation_changed(window.is_window_active());
        cx.notify();
    }

    pub(super) fn take_bot_ask(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((bot, (ask, by))) = self.store.update(cx, |s, _| s.take_asked(|asks| &mut asks.bot_ask)) else {
            return;
        };
        let in_field = bot_chat::field_focus(cx).is_focused(window);
        let s = self.store.read(cx);
        let here = Here {
            shown: s.shown_bot() == Some(&bot.key()),
            active: window.is_window_active(),
            covered: !s.chat_takes_keys(),
            keys: keys_of(in_field, !sendable(s.bots.drafts.text(&bot.id))),
        };
        if opens(by, here, s.dialog.is_some()) {
            dialogs::answer_ask(&bot, ask, window, cx);
        }
    }

    pub(crate) fn chat_shown(&self) -> bool {
        self.side.bot.is_some()
    }

    pub(crate) fn bots_shown(&self) -> bool {
        self.side.bots
    }
}
