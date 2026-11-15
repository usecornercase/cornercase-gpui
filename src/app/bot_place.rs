use gpui::{Context, Entity, Window};

use super::Root;
use crate::store::Store;

impl Root {
    pub(super) fn follow_place(&mut self, store: &Entity<Store>, window: &mut Window, cx: &mut Context<Self>) {
        let (place, bot) = {
            let s = store.read(cx);
            (s.machine_place().cloned(), s.shown_bot().map(str::to_string))
        };
        let was = self.side.bot.is_some();
        let moved = bot != self.side.bot;
        let rested = self.keys_rest(cx);
        self.side.bot = bot;
        if place != self.side.machine_place {
            self.machine_place_changed(place, window, cx);
        }
        if moved {
            self.chat_place_changed(was, window, cx);
            self.keys_follow(&rested, window, cx);
        }
    }

    fn chat_place_changed(&mut self, was: bool, window: &Window, cx: &mut Context<Self>) {
        match (was, self.side.bot.is_some()) {
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

    pub(crate) fn chat_shown(&self) -> bool {
        self.side.bot.is_some()
    }
}
