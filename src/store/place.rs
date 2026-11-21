use gpui::Context;

use super::Store;
use crate::model::MachinePlace;
use crate::model::bots::BotKey;
use crate::model::place::{Moved, Showing};

#[derive(Default)]
pub(super) struct Places {
    pub(super) showing: Showing,
    pub(super) last_bot: Option<BotKey>,
}

impl Store {
    pub(crate) fn machine_place(&self) -> Option<&MachinePlace> {
        self.places.showing.machine()
    }

    pub(crate) fn shown_bot(&self) -> Option<&BotKey> {
        self.places.showing.bot()
    }

    pub(crate) fn last_bot(&self) -> Option<&BotKey> {
        self.places.last_bot.as_ref()
    }

    pub(crate) fn shows_local(&self) -> bool {
        self.places.showing == Showing::Local
    }

    pub(super) fn place(&mut self, moved: Moved) {
        let before = self.shown_bot().cloned();
        self.places.showing = std::mem::take(&mut self.places.showing).after(moved);
        if before.as_ref() != self.shown_bot() {
            self.bots.chat = None;
            self.there.bots.forget_chats();
            self.bots.bump();
        }
    }

    pub(super) fn forget_awaited(&mut self) {
        self.placing.awaited = None;
        self.bots.manage.person_moved();
    }

    pub(crate) fn show_local(&mut self, cx: &mut Context<Self>) {
        self.forget_awaited();
        if !self.shows_local() {
            self.place(Moved::Local);
            self.file_full_screen = false;
            cx.notify();
        }
    }
}
