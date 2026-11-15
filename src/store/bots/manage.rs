use std::time::Instant;

use gpui::{Context, Pixels, Point};

use super::super::run_on::start_key;
use super::super::{Busy, Store, ToastKind};
use super::talk::{Call, Then};
use crate::cli::{self, Feature};
use crate::model::bots::{
    Arrival, Creating, Making, create_failed, id_of, making, rename_args, rename_failed, save_failed,
};
use crate::platform::Os;

fn remove_key(id: &str) -> String {
    format!("bot-remove:{id}")
}

impl Store {
    pub(crate) fn making(&self) -> Option<Making> {
        self.session.as_ref()?;
        Some(making(self.releases().lacks(Feature::Bots), Os::CURRENT))
    }

    fn dialog_job(&mut self, (dialog, doing): (u64, String), args: Vec<String>, then: Then, cx: &mut Context<Self>) {
        if self.busy(&start_key(dialog)) {
            return;
        }
        self.bot_job(Busy::new(start_key(dialog), doing), Call::args(args), then, cx);
    }

    pub(crate) fn create_bot(&mut self, args: Vec<String>, name: &str, dialog: u64, cx: &mut Context<Self>) {
        let creating = Creating::new(dialog, name, self.bots.boot.as_deref());
        let then: Then = Box::new(move |this, result, cx| this.bot_created(&creating, result, cx));
        self.dialog_job((dialog, format!("Creating the bot {name}…")), args, then, cx);
    }

    pub(crate) fn save_bot(&mut self, args: Vec<String>, name: &str, dialog: u64, cx: &mut Context<Self>) {
        let name_of = name.to_string();
        let then: Then = Box::new(move |this, result, cx| this.bot_saved(dialog, &name_of, result, cx));
        self.dialog_job((dialog, format!("Saving {name}…")), args, then, cx);
    }

    fn bot_saved(&mut self, dialog: u64, name: &str, result: anyhow::Result<String>, cx: &mut Context<Self>) {
        match result {
            Ok(_) => self.close_started(Some(dialog), false, cx),
            Err(error) => self.refused_here(dialog, save_failed(name, &cli::reason(&error)), cx),
        }
    }

    pub(crate) fn rename_bot(&mut self, id: &str, name: &str, cx: &mut Context<Self>) {
        let was = self.bot_name(id);
        let busy = Busy::new(format!("bot-rename:{id}"), format!("Renaming {was}…"));
        let then: Then = Box::new(move |this, result, cx| {
            if let Err(error) = result {
                this.toast(rename_failed(&was, &cli::reason(&error)), ToastKind::Error, cx);
            }
        });
        self.bot_job(busy, Call::args(rename_args(id, name)), then, cx);
    }

    pub(crate) fn ask_bot_menu(&mut self, id: &str, at: Point<Pixels>, cx: &mut Context<Self>) {
        self.asks.bot_menu.ask(id, at);
        cx.notify();
    }

    fn bot_created(&mut self, creating: &Creating, result: anyhow::Result<String>, cx: &mut Context<Self>) {
        let dialog = creating.dialog;
        match result {
            Ok(answer) => {
                let shown = self.shown_dialog() == Some(dialog);
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

    fn refused_here(&mut self, dialog: u64, words: String, cx: &mut Context<Self>) {
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
            Arrival::Listed => self.show_bot(&made.id, cx),
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
        let (name, bot) = (self.bot_name(id), id.to_string());
        let busy = Busy::new(remove_key(id), format!("Removing {name}…"));
        let then: Then = Box::new(move |this, result, cx| {
            if let Err(error) = result {
                this.bots.manage.removal_failed(&bot);
                this.toast(format!("Could not remove {name}: {}", cli::reason(&error)), ToastKind::Error, cx);
            }
        });
        self.bot_job(busy, Call::of(&["bot", "remove", id]), then, cx);
    }

    pub(crate) fn names_bot(&mut self, id: &str) {
        if let Some(dialog) = self.shown_dialog() {
            self.bots.manage.name_dialog(id, dialog);
        }
    }

    pub(super) fn close_dialog_if_gone(&mut self, listed: &[String], cx: &mut Context<Self>) {
        if self.bots.manage.named_gone(listed, self.shown_dialog()) {
            self.close_dialog(cx);
        }
    }

    pub(super) fn close_dialog_naming(&mut self, gone: &str, cx: &mut Context<Self>) {
        if self.bots.manage.closes_dialog(gone, self.shown_dialog()) {
            self.close_dialog(cx);
        }
    }
}
