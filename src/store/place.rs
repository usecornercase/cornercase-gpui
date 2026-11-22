use gpui::Context;

use super::Store;
use crate::model::MachinePlace;
use crate::model::bots::{BotKey, Offer};
use crate::model::place::{Moved, Showing};

#[derive(Default)]
pub(super) struct Places {
    pub(super) showing: Showing,
    pub(super) last_bot: Option<BotKey>,
    back: Option<MachinePlace>,
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

    pub(crate) fn shows_bots(&self) -> bool {
        self.places.showing.bots()
    }

    pub(super) fn place(&mut self, moved: Moved) {
        let before = self.shown_bot().cloned();
        let left = std::mem::take(&mut self.places.showing);
        let from = left.machine().cloned();
        self.places.showing = left.after(moved);
        if !self.shows_bots() {
            self.places.back = None;
        } else if from.is_some() {
            self.places.back = from;
        }
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

    pub(crate) fn toggle_bots(&mut self, cx: &mut Context<Self>) {
        if self.shows_bots() {
            self.leave_bots(cx);
        } else {
            self.open_bots(cx);
        }
    }

    pub(crate) fn open_bots(&mut self, cx: &mut Context<Self>) {
        if self.shows_bots() {
            return;
        }
        if let Some(key) = self.bot_to_open() {
            self.show_bot(&key, cx);
            return;
        }
        self.forget_awaited();
        self.place(Moved::Bots);
        self.file_full_screen = false;
        cx.notify();
    }

    fn bot_to_open(&self) -> Option<BotKey> {
        if self.bots_offer() != Offer::Rows {
            return None;
        }
        let bots = self.all_bots();
        let last = self.last_bot().and_then(|key| bots.iter().find(|bot| bot.key() == *key));
        last.or(bots.first()).map(|bot| bot.key())
    }

    fn leave_bots(&mut self, cx: &mut Context<Self>) {
        let machines = self.session.as_ref().map(|session| session.machines.as_slice()).unwrap_or_default();
        match self.places.back.as_ref().and_then(|place| place.followed(machines)) {
            Some(place) => {
                self.pick_machine_place(place, cx);
                cx.notify();
            }
            None => self.show_local(cx),
        }
    }
}
