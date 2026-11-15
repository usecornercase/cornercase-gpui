use std::time::Instant;

use super::super::run_on::start_key;
use super::super::{Busy, Store, ToastKind};
use super::talk::{Call, Then};
use crate::cli::{self, Feature};
use crate::model::bots::{Arrival, Making, id_of, made_when, making};
use crate::platform::Os;
use gpui::Context;

fn remove_key(id: &str) -> String {
    format!("bot-remove:{id}")
}

pub(crate) fn create_failed(name: &str, words: &str) -> String {
    format!("Could not create the bot {name}: {words}")
}

impl Store {
    pub(crate) fn making(&self) -> Option<Making> {
        self.session.as_ref()?;
        Some(making(self.releases().lacks(Feature::Bots), Os::CURRENT))
    }

    pub(crate) fn create_bot(&mut self, args: Vec<String>, name: &str, dialog: u64, cx: &mut Context<Self>) {
        if self.busy(&start_key(dialog)) {
            return;
        }
        let name = name.to_string();
        let busy = Busy::new(start_key(dialog), format!("Creating the bot {name}…"));
        let then: Then = Box::new(move |this, result, cx| this.bot_created(dialog, &name, result, cx));
        self.bot_job(busy, Call::args(args), then, cx);
    }

    fn bot_created(&mut self, dialog: u64, name: &str, result: anyhow::Result<String>, cx: &mut Context<Self>) {
        match result {
            Ok(answer) => {
                let shown = self.shown_dialog() == Some(dialog);
                self.close_started(Some(dialog), false, cx);
                match id_of(&answer) {
                    Some(id) => {
                        let boot = self.bots.boot.as_deref();
                        self.bots.manage.made = made_when(shown, &id, boot, Instant::now());
                        self.follow_made(cx);
                    }
                    None => self.toast("cornercase bot new printed no id", ToastKind::Error, cx),
                }
            }
            Err(error) => self.create_refused(dialog, create_failed(name, &cli::reason(&error)), cx),
        }
    }

    fn create_refused(&mut self, dialog: u64, words: String, cx: &mut Context<Self>) {
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
