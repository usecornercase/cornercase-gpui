use gpui::Context;

use super::super::{Busy, Store, ToastKind};
use super::talk::{Call, Then};
use crate::cli::{self, Feature};
use crate::model::bots::{self, AskOpen, By, Give, Told, decline_args, give_args, giving, tells_ask};
use crate::platform::Os;

fn decline_key(bot: &str, ask: u64) -> String {
    format!("bot-decline:{bot}:{ask}")
}

impl Store {
    pub(crate) fn asks_offer(&self) -> Give {
        giving(self.releases().lacks(Feature::BotAsks), Os::CURRENT)
    }

    fn waits(&self, bot: &str, ask: u64) -> bool {
        self.bots.find(bot).is_some_and(|found| found.asks.iter().any(|waiting| waiting.id == ask))
    }

    pub(super) fn asks_moved(&mut self, arrived: &[(String, u64)], cx: &mut Context<Self>) {
        let shown = self.shown_dialog();
        if self.bots.asking.open.as_ref().is_some_and(|open| open.ended(&self.bots.list, shown)) {
            self.bots.asking.open = None;
            self.close_dialog(cx);
        }
        self.drop_ended_ask_toasts(cx);
        if self.asks_offer() != Give::Offered {
            return;
        }
        let on_screen = (self.shown_bot().map(str::to_string), self.bots.active);
        let mut opened = false;
        let mut told = Vec::new();
        for (bot, ask) in arrived {
            if on_screen.1 && on_screen.0.as_deref() == Some(bot.as_str()) && !opened {
                self.asks.bot_ask.ask(bot, (*ask, By::Itself));
                opened = true;
            } else if tells_ask(bot, (on_screen.0.as_deref(), on_screen.1), true) {
                told.extend(self.asking_told(bot, *ask));
            }
        }
        if opened || !told.is_empty() {
            self.bots.to_tell.extend(told);
            cx.notify();
        }
    }

    fn asking_told(&self, bot: &str, ask: u64) -> Option<Told> {
        let found = self.bots.find(bot)?;
        let waiting = found.asks.iter().find(|waiting| waiting.id == ask)?;
        Some(Told::asking(bot, &found.name, waiting))
    }

    pub(crate) fn open_ask(&mut self, bot: &str, ask: u64, cx: &mut Context<Self>) {
        self.asks.bot_ask.ask(bot, (ask, By::Person));
        cx.notify();
    }

    pub(in crate::store) fn go_to_ask(&mut self, id: &str, name: &str, ask: u64, cx: &mut Context<Self>) {
        if self.bots.find(id).is_none() {
            self.toast(bots::gone(name), ToastKind::Info, cx);
            return;
        }
        self.show_bot(id, cx);
        if self.waits(id, ask) && self.asks_offer() == Give::Offered {
            self.open_ask(id, ask, cx);
        }
    }

    pub(crate) fn names_ask(&mut self, bot: &str, ask: u64) {
        if let Some(dialog) = self.shown_dialog() {
            self.bots.asking.open = Some(AskOpen { bot: bot.to_string(), ask, dialog });
        }
    }

    pub(crate) fn give_secrets(&mut self, (bot, ask): (&str, u64), json: String, dialog: u64, cx: &mut Context<Self>) {
        let doing = format!("Saving the secrets of {}…", self.bot_name(bot));
        let then: Then = Box::new(move |this, result, cx| match result {
            Ok(_) => this.close_started(Some(dialog), false, cx),
            Err(error) => this.refused_here(dialog, format!("Could not save the secrets: {}", cli::reason(&error)), cx),
        });
        self.dialog_job((dialog, doing), Call::with_stdin(give_args(bot, ask), json.into_bytes()), then, cx);
    }

    pub(crate) fn decline_ask(&mut self, (bot, ask): (&str, u64), cx: &mut Context<Self>) {
        if self.busy(&decline_key(bot, ask)) {
            return;
        }
        let name = self.bot_name(bot);
        let busy = Busy::new(decline_key(bot, ask), format!("Declining what {name} asked for…"));
        let then: Then = Box::new(move |this, result, cx| {
            if let Err(error) = result {
                let words = format!("Could not decline what {name} asked for: {}", cli::reason(&error));
                this.toast(words, ToastKind::Error, cx);
            }
        });
        self.bot_job(busy, Call::args(decline_args(bot, ask)), then, cx);
    }
}
