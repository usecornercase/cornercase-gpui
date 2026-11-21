use std::time::Instant;

use gpui::{Context, Pixels, Point};

use super::super::run_on::start_key;
use super::super::{Asks, Busy, Store, ToastKind, args};
use super::talk::{Call, Then};
use crate::cli::{self, Feature};
use crate::model::bots::{
    AboutGate, Arrival, Bot, BotKey, Closes, Creating, Home, Making, MenuAsk, about_gate, create_failed, id_of, making,
    remove_failed, rename_args, rename_failed, save_failed,
};
use crate::model::place::awaits;
use crate::platform::Os;

fn remove_key(id: &str) -> String {
    format!("bot-remove:{id}")
}

fn rename_key(id: &str) -> String {
    format!("bot-rename:{id}")
}

impl Store {
    pub(crate) fn about_gate(&self, home: &Home) -> AboutGate {
        about_gate(self.bot_releases(home).lacks(Feature::BotAboutRead), Os::CURRENT)
    }

    pub(crate) fn read_bot_about(
        &mut self,
        key: &BotKey,
        then: impl FnOnce(&mut Self, anyhow::Result<String>, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        let busy =
            Busy::new(format!("bot-about:{}", key.tag()), format!("Reading what {} is for…", self.bot_label(key)));
        self.bot_read(busy, Call::about(key), Box::new(then), cx);
    }

    pub(crate) fn making(&self) -> Option<Making> {
        self.session.as_ref()?;
        Some(making(self.releases().lacks(Feature::Bots), Os::CURRENT))
    }

    pub(super) fn dialog_job(
        &mut self,
        (dialog, doing): (u64, String),
        call: Call,
        then: Then,
        cx: &mut Context<Self>,
    ) {
        if self.busy(&start_key(dialog)) {
            return;
        }
        self.bot_job(Busy::new(start_key(dialog), doing), call, then, cx);
    }

    pub(crate) fn create_bot(&mut self, args: Vec<String>, name: &str, dialog: u64, cx: &mut Context<Self>) {
        let creating = Creating::new(dialog, name, self.bots.boot.as_deref());
        let then: Then = Box::new(move |this, result, cx| this.bot_created(&creating, result, cx));
        self.dialog_job((dialog, format!("Creating the bot {name}…")), Call::here(args), then, cx);
    }

    pub(crate) fn save_bot(&mut self, args: Vec<String>, name: &str, dialog: u64, cx: &mut Context<Self>) {
        let name_of = name.to_string();
        let then: Then = Box::new(move |this, result, cx| this.bot_saved(dialog, &name_of, result, cx));
        self.dialog_job((dialog, format!("Saving {name}…")), Call::here(args), then, cx);
    }

    fn bot_saved(&mut self, dialog: u64, name: &str, result: anyhow::Result<String>, cx: &mut Context<Self>) {
        match result {
            Ok(_) => self.close_started(Some(dialog), false, cx),
            Err(error) => self.refused_here(dialog, save_failed(name, &cli::reason(&error)), cx),
        }
    }

    pub(crate) fn rename_bot(&mut self, id: &str, name: &str, cx: &mut Context<Self>) {
        let was = self.bot_label(&BotKey::here(id));
        let busy = Busy::new(rename_key(id), format!("Renaming {was}…"));
        let then: Then = Box::new(move |this, result, cx| {
            if let Err(error) = result {
                this.toast(rename_failed(&was, &cli::reason(&error)), ToastKind::Error, cx);
            }
        });
        self.bot_job(busy, Call::here(rename_args(id, name)), then, cx);
    }

    pub(crate) fn ask_bot_menu(&mut self, key: &BotKey, at: Point<Pixels>, keys: bool, cx: &mut Context<Self>) {
        self.asks.bot_menu.ask(key, (at, keys));
        cx.notify();
    }

    pub(crate) fn take_asked<P>(&mut self, pick: impl FnOnce(&mut Asks) -> &mut MenuAsk<P>) -> Option<(Bot, P)> {
        let mut asked = std::mem::take(pick(&mut self.asks));
        asked.take(|key| self.bot(key).cloned())
    }

    fn bot_created(&mut self, creating: &Creating, result: anyhow::Result<String>, cx: &mut Context<Self>) {
        let dialog = creating.dialog;
        match result {
            Ok(answer) => {
                let shown = awaits(true, Some(dialog), self.shown_dialog());
                self.close_started(Some(dialog), false, cx);
                match id_of(&answer) {
                    Some(id) => {
                        self.bots.manage.made = creating.made(shown, &id, Instant::now());
                        self.follow_made(cx);
                    }
                    None => self.toast("cornercase bot new printed no id", ToastKind::Error, cx),
                }
            }
            Err(error) => self.refused_here(dialog, create_failed(&creating.name, &cli::reason(&error)), cx),
        }
    }

    pub(super) fn refused_here(&mut self, dialog: u64, words: String, cx: &mut Context<Self>) {
        if self.shown_dialog() == Some(dialog) {
            self.bots.manage.refuse(dialog, words);
            cx.notify();
        } else {
            self.toast(words, ToastKind::Error, cx);
        }
    }

    pub(super) fn follow_made(&mut self, cx: &mut Context<Self>) {
        let Some(made) = self.bots.manage.made.take() else { return };
        match made.arrival(&self.bots.list, self.bots.boot.as_deref(), Instant::now()) {
            Arrival::Listed => self.show_bot(&BotKey::here(&made.id), cx),
            Arrival::NotYet => self.bots.manage.made = Some(made),
            Arrival::Gone => {}
        }
    }

    pub(crate) fn forget_bot_refusal(&mut self, dialog: u64, cx: &mut Context<Self>) {
        if self.bots.manage.forget_refusal(dialog) {
            cx.notify();
        }
    }

    pub(crate) fn remove_bot(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.busy(&remove_key(id)) {
            return;
        }
        self.bots.manage.asked_to_remove(id);
        let (name, bot) = (self.bot_label(&BotKey::here(id)), id.to_string());
        let busy = Busy::new(remove_key(id), format!("Removing {name}…"));
        let then: Then = Box::new(move |this, result, cx| {
            if let Err(error) = result {
                this.bots.manage.removal_failed(&bot);
                this.toast(remove_failed(&name, &cli::reason(&error)), ToastKind::Error, cx);
            }
        });
        self.bot_job(busy, Call::here(args(&["bot", "remove", id])), then, cx);
    }

    pub(crate) fn names_bot(&mut self, key: &BotKey) {
        let Some(dialog) = self.shown_dialog() else { return };
        if let Some(bots) = self.bots_at_mut(&key.home) {
            bots.manage.name_dialog(&key.id, dialog);
        }
    }

    pub(super) fn close_dialog_if_gone(&mut self, home: &Home, listed: &[String], cx: &mut Context<Self>) {
        let (shown, behind) = (self.shown_dialog(), self.behind_dialog());
        let closes = self.bots_at(home).map_or(Closes::Nothing, |bots| bots.manage.gone_closes(listed, shown, behind));
        self.close_named(closes, cx);
    }

    pub(super) fn close_dialog_naming(&mut self, gone: &BotKey, cx: &mut Context<Self>) {
        let (shown, behind) = (self.shown_dialog(), self.behind_dialog());
        let closes =
            self.bots_at(&gone.home).map_or(Closes::Nothing, |bots| bots.manage.closes_dialog(&gone.id, shown, behind));
        self.close_named(closes, cx);
    }

    fn close_named(&mut self, closes: Closes, cx: &mut Context<Self>) {
        match closes {
            Closes::Nothing => {}
            Closes::Shown => self.close_dialog(cx),
            Closes::Both => self.close_dialog_and_behind(cx),
        }
    }
}
