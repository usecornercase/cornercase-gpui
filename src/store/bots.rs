use gpui::Context;
use serde_json::Value;

use super::{Busy, Store, ToastKind, args, run_job};
use crate::cli::{self, Feature};
use crate::model::bots::{self, BotEvent, CatchUp, Chat, Heard, Offer, Read};
use crate::model::place::Moved;
use crate::platform::Os;

mod manage;
mod talk;

impl Store {
    pub(crate) fn bots_offer(&self) -> Offer {
        self.bots.offer(self.releases().lacks(Feature::Bots), Os::CURRENT)
    }

    pub(crate) fn bots_revision(&self) -> u64 {
        self.bots.revision()
    }

    pub(super) fn take_bots(&mut self, app: &Value, cx: &mut Context<Self>) {
        let shown = self.shown_bot().and_then(|id| self.bots.find(id)).map(|bot| (bot.id.clone(), bot.name.clone()));
        let draft = shown.as_ref().map(|(id, _)| self.bots.drafts.text(id).to_string()).unwrap_or_default();
        let asked = shown.as_ref().is_some_and(|(id, _)| self.bots.manage.asked(id));
        let moved = self.bots.take_app(app);
        self.place(Moved::Listed(moved.listed.clone()));
        self.close_dialog_if_gone(&moved.listed, cx);
        match shown {
            Some((_, name)) if self.shown_bot().is_none() => {
                if !asked {
                    self.toast_with(format!("{name} was removed"), ToastKind::Info, talk::copy_action(&draft), cx);
                }
            }
            _ if moved.boot_changed && self.bots.chat.is_some() => self.read_chat(None, cx),
            _ => self.heal_chat(&moved.settled, cx),
        }
        self.mind_seen(cx);
        self.follow_made(cx);
    }

    fn heal_chat(&mut self, settled: &[String], cx: &mut Context<Self>) {
        if self.bots.chat.as_ref().is_some_and(|chat| settled.contains(&chat.bot)) {
            self.catch_up_chat(cx);
        }
    }

    pub(crate) fn show_bot(&mut self, id: &str, cx: &mut Context<Self>) {
        if !self.bots.can_show(id, &self.bots_offer()) {
            return;
        }
        if self.shown_bot() != Some(id) {
            self.forget_awaited();
            self.bots.forget_seen();
            self.place(Moved::Bot(id.to_string()));
            let asked = self.bots.next_ask();
            self.bots.chat = Some(Chat::new(id, asked));
            self.read_chat(None, cx);
        }
        self.catch_up_chat(cx);
        self.give_keys_back(cx);
        self.mind_seen(cx);
        cx.notify();
    }

    pub(crate) fn bot_heard(&mut self, event: &BotEvent, cx: &mut Context<Self>) {
        match event.heard() {
            Heard::Line(bot, line) => self.line_heard(&bot, line, cx),
            Heard::State(bot, thinking) => {
                if self.bots.state_heard(&bot, thinking) {
                    cx.notify();
                }
            }
            Heard::Removed(bot) => self.bot_removed(&bot, cx),
            Heard::Nothing => {}
        }
    }

    fn line_heard(&mut self, bot: &str, line: bots::Line, cx: &mut Context<Self>) {
        match self.bots.heard(bot, line) {
            Some(bots::Added::Yes) => cx.notify(),
            Some(bots::Added::Gap(last)) => self.read_chat(Some(last), cx),
            Some(bots::Added::No) | None => {}
        }
        if self.bots.chat.as_ref().is_some_and(|chat| chat.bot == bot && chat.failed()) {
            self.catch_up_chat(cx);
        }
        self.mind_seen(cx);
    }

    fn bot_removed(&mut self, bot: &str, cx: &mut Context<Self>) {
        let shown = self.shown_bot() == Some(bot);
        let name = self.bots.find(bot).map_or_else(|| bot.to_string(), |found| found.name.clone());
        let draft = self.bots.drafts.take(bot).unwrap_or_default();
        self.place(Moved::Removed(bot.to_string()));
        if self.bots.manage.told(bot, shown) {
            self.toast_with(format!("{name} was removed"), ToastKind::Info, talk::copy_action(&draft), cx);
        }
        self.close_dialog_naming(bot, cx);
        cx.notify();
    }

    pub(crate) fn bot_stream_started(&mut self, cx: &mut Context<Self>) {
        self.catch_up_chat(cx);
    }

    fn catch_up_chat(&mut self, cx: &mut Context<Self>) {
        match self.bots.chat.as_ref().map(Chat::catch_up) {
            Some(CatchUp::After(last)) => self.read_chat(Some(last), cx),
            Some(CatchUp::Whole) => self.read_chat(None, cx),
            Some(CatchUp::Nothing) | None => {}
        }
    }

    pub(crate) fn bots_window_active(&mut self, active: bool, cx: &mut Context<Self>) {
        if self.bots.active != active {
            self.bots.active = active;
            self.mind_seen(cx);
        }
    }

    fn read_chat(&mut self, after: Option<u64>, cx: &mut Context<Self>) {
        let Some(chat) = self.bots.chat.as_ref() else { return };
        let bot = chat.bot.clone();
        let key = format!("bot-history:{bot}");
        if let (Some(from), true) = (after, self.busy(&key)) {
            if let Some(chat) = self.bots.chat.as_mut() {
                chat.defer(from);
            }
            return;
        }
        let asked = if after.is_none() { self.bots.next_ask() } else { chat.asked };
        if let Some(chat) = self.bots.chat.as_mut().filter(|_| after.is_none()) {
            chat.asked = asked;
            chat.read = Read::Asking;
        }
        let name = self.bots.find(&bot).map_or_else(|| bot.clone(), |found| found.name.clone());
        let from = after.map(|last| last.to_string());
        let mut words = vec!["bot", "history", &bot, "--json"];
        match from.as_deref() {
            Some(last) => words.extend(["--after", last]),
            None => words.extend(["--last", "1000"]),
        }
        let busy = Busy::new(key, format!("Reading the chat of {name}…"));
        let id = self.start_job(busy, cx);
        run_job(cx, id, args(&words), move |this, result, cx| this.chat_read((&bot, asked), result, cx));
    }

    fn chat_read(&mut self, (bot, asked): (&str, u64), result: anyhow::Result<String>, cx: &mut Context<Self>) {
        let name = self.bots.find(bot).map_or_else(|| bot.to_string(), |found| found.name.clone());
        let Some(chat) = self.bots.chat.as_mut().filter(|chat| chat.awaits(bot, asked)) else { return };
        match result.and_then(|out| bots::history_of(&out)) {
            Ok(lines) => chat.take(lines),
            Err(error) => {
                let reason = cli::reason(&error);
                chat.fail(reason.clone());
                self.toast(format!("Could not read the chat of {name}: {reason}"), ToastKind::Error, cx);
            }
        }
        let pending = self.bots.chat.as_mut().and_then(Chat::take_pending);
        if let Some(from) = pending {
            self.read_chat(Some(from), cx);
        }
        self.mind_seen(cx);
        cx.notify();
    }

    pub(super) fn mind_seen(&mut self, _cx: &mut Context<Self>) {
        let Some(id) = self.bots.seen_due(self.places.showing.bot()) else { return };
        let unread = self.bots.find(&id).map_or(0, |bot| bot.unread);
        self.bots.seen = Some((id.clone(), unread));
        self.cmd(args(&["bot", "seen", &id]));
    }
}
