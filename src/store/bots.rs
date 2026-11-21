use std::time::Instant;

use gpui::Context;
use serde_json::Value;

use super::{Busy, Store, ToastKind};
use crate::cli::{self, Feature};
use crate::model::bots::{
    self, Anchor, Answer, Bot, BotEvent, BotKey, Bots, CatchUp, Chat, Heard, Home, Offer, Read, Runs, Told,
};
use crate::model::machines::run_on::server_release;
use crate::model::place::Moved;
use crate::panels::queue;
use crate::platform::Os;

use self::talk::Call;

mod asks;
mod follow;
mod manage;
mod routines;
mod secrets;
mod talk;
mod there;

pub(super) use self::there::BotsThere;

fn take_runs(bots: &mut Bots, app: &Value) {
    let runs = Runs::of(&queue::entries(Some(app)));
    if runs != bots.runs {
        bots.runs = runs;
        bots.bump();
    }
}

struct Before {
    shown: Option<String>,
    draft: String,
    asked: bool,
    unread: Vec<(String, usize)>,
}

pub(super) use routines::closes;

impl Store {
    pub(crate) fn bots_offer(&self) -> Offer {
        bots::offer_of(self.any_bot_listed(), self.releases().lacks(Feature::Bots), Os::CURRENT)
    }

    pub(crate) fn bot_setup_offered(&self, key: &BotKey) -> bool {
        let releases = match key.home.machine() {
            None => self.releases(),
            Some(machine) => cli::Releases {
                server: server_release(Some(machine), self.session.as_ref(), &self.machines),
                here: self.heard_release(),
                machine: None,
            },
        };
        releases.lacks(Feature::BotSetup).is_none()
    }

    pub(crate) fn chat_apps_row(&self) -> bots::Row {
        bots::row(self.releases().lacks(Feature::BotChatApps), &bots::platforms(self.app.as_ref()), Os::CURRENT)
    }

    pub(crate) fn bot_reach(&self, key: &BotKey) -> Option<bots::Reach> {
        let bot = self.bot(key).filter(|bot| bot.home == Home::Here)?;
        bots::reach(&bot.chat_apps, &bots::platforms(self.app.as_ref()))
    }

    pub(crate) fn bots_revision(&self) -> u64 {
        self.bots.revision() + self.there.bots.revision()
    }

    pub(crate) fn chat_takes_keys(&self) -> bool {
        bots::keys_go_to_chat(self.dialog.is_some(), self.sheet.is_some()) && !self.shows_file_full_screen()
    }

    fn listed_before(&self, home: &Home) -> Option<Before> {
        let bots = self.bots_at(home)?;
        let shown = self.shown_bot().filter(|key| key.home == *home).and_then(|key| bots.find(&key.id));
        Some(Before {
            shown: shown.map(Bot::label),
            draft: shown.map(|bot| bots.drafts.text(&bot.id).to_string()).unwrap_or_default(),
            asked: shown.is_some_and(|bot| bots.manage.asked(&bot.id)),
            unread: bots.unread_of(),
        })
    }

    pub(super) fn take_bots(&mut self, home: &Home, app: &Value, cx: &mut Context<Self>) {
        let Some(before) = self.listed_before(home) else { return };
        let Some(bots) = self.bots_at_mut(home) else { return };
        let moved = bots.take_app(app);
        let arrived = bots.asking.known.arrived(&bots.list);
        take_runs(bots, app);
        self.place(Moved::Listed(home.clone(), moved.listed.clone()));
        self.close_dialog_if_gone(home, &moved.listed, cx);
        if *home == Home::Here {
            self.close_routines_if_gone(cx);
            self.asks_moved(&arrived, cx);
        }
        let chat = self.bots_at(home).is_some_and(|bots| bots.chat.is_some());
        match before.shown {
            Some(label) if self.shown_bot().is_none() => {
                if !before.asked {
                    let copy = talk::copy_action(&before.draft);
                    self.toast_with(format!("{label} was removed"), ToastKind::Info, copy, cx);
                }
            }
            _ if moved.boot_changed && chat => self.read_chat(None, cx),
            _ => self.heal_chat(home, &moved.settled, cx),
        }
        self.answers_counted(home, &before.unread, (moved.boot_changed, &arrived), cx);
        self.mind_seen(cx);
        if *home == Home::Here {
            self.follow_made(cx);
        }
    }

    fn answers_counted(
        &mut self,
        home: &Home,
        before: &[(String, usize)],
        (boot, arrived): (bool, &[(String, u64)]),
        cx: &mut Context<Self>,
    ) {
        let Some(bots) = self.bots_at_mut(home) else { return };
        if boot {
            bots.answers.forget();
            return;
        }
        let now = Instant::now();
        let (mut due, mut read) = (Vec::new(), Vec::new());
        for (id, was) in before {
            let is = bots.find(id).map(|bot| bot.unread);
            if is.is_none_or(|is| bots::read_since(*was, is)) {
                read.push(BotKey { home: home.clone(), id: id.clone() });
            }
            if let Some(is) = is {
                let asked = arrived.iter().filter(|(bot, _)| bot == id).count();
                due.extend(bots.answers.counted(id, bots::counted_as(*was, is, asked), now));
            }
        }
        for key in &read {
            self.drop_bot_toasts(key, cx);
        }
        self.tell_answers(home, &due, cx);
    }

    fn answer_heard(&mut self, home: &Home, (bot, line): (&str, &bots::Line), cx: &mut Context<Self>) {
        let Some(answer) = bots::answer_of(bot, line) else { return };
        let due = self.bots_at_mut(home).and_then(|bots| bots.answers.heard(answer, Instant::now()));
        if let Some(due) = due {
            self.tell_answers(home, &[due], cx);
        }
    }

    fn tell_answers(&mut self, home: &Home, answers: &[Answer], cx: &mut Context<Self>) {
        let offered = self.bots_offer() == Offer::Rows;
        let shown = self.shown_bot().filter(|key| key.home == *home).map(|key| key.id.as_str());
        let told: Vec<Told> = answers
            .iter()
            .filter(|answer| bots::tells(answer, (shown, self.bots.active), offered))
            .map(|answer| {
                let key = BotKey { home: home.clone(), id: answer.bot.clone() };
                let label = self.bot_label(&key);
                let run = answer.run.and_then(|run| self.bots_at(home).and_then(|bots| bots.runs.label(run)));
                Told::new(answer, key, &label, run)
            })
            .collect();
        if told.is_empty() {
            return;
        }
        self.bots.to_tell.extend(told);
        cx.notify();
    }

    pub(super) fn go_to_bot(&mut self, key: &BotKey, name: &str, run: Option<u64>, cx: &mut Context<Self>) {
        if self.bot(key).is_some() {
            self.show_bot_at(key, run, cx);
        } else {
            self.toast(bots::gone(name), ToastKind::Info, cx);
        }
    }

    fn heal_chat(&mut self, home: &Home, settled: &[String], cx: &mut Context<Self>) {
        let chat = self.bots_at(home).and_then(|bots| bots.chat.as_ref());
        if chat.is_some_and(|chat| settled.contains(&chat.bot)) {
            self.catch_up_chat(cx);
        }
    }

    pub(crate) fn show_bot(&mut self, key: &BotKey, cx: &mut Context<Self>) {
        let offer = self.bots_offer();
        if !self.bots_at(&key.home).is_some_and(|bots| bots.can_show(&key.id, &offer)) {
            return;
        }
        self.places.last_bot = Some(key.clone());
        if self.shown_bot() != Some(key) {
            self.forget_awaited();
            self.place(Moved::Bot(key.clone()));
            if let Some(bots) = self.bots_at_mut(&key.home) {
                bots.forget_seen();
                let asked = bots.next_ask();
                bots.chat = Some(Chat::new(&key.id, asked));
            }
            self.read_chat(None, cx);
        }
        self.catch_up_chat(cx);
        self.file_full_screen = false;
        if bots::keys_go_to_chat(self.dialog.is_some(), self.sheet.is_some()) {
            self.ask_for_the_terminal(cx);
        }
        self.mind_seen(cx);
        cx.notify();
    }

    pub(crate) fn show_bot_at(&mut self, key: &BotKey, run: Option<u64>, cx: &mut Context<Self>) {
        self.show_bot(key, cx);
        let Some(run) = run.filter(|_| self.shown_bot() == Some(key)) else { return };
        let Some(bots) = self.bots_at_mut(&key.home) else { return };
        let serial = bots.next_ask();
        bots.anchor = Some(Anchor { bot: key.id.clone(), run, serial });
        bots.bump();
        cx.notify();
    }

    pub(crate) fn bot_heard_here(&mut self, event: &BotEvent, cx: &mut Context<Self>) {
        self.bot_heard(&Home::Here, event, cx);
    }

    pub(super) fn bot_heard(&mut self, home: &Home, event: &BotEvent, cx: &mut Context<Self>) {
        match event.heard() {
            Heard::Line(bot, line) => {
                self.answer_heard(home, (&bot, &line), cx);
                self.line_heard(home, &bot, line, cx);
            }
            Heard::State(bot, thinking) => {
                if self.bots_at_mut(home).is_some_and(|bots| bots.state_heard(&bot, thinking)) {
                    cx.notify();
                }
            }
            Heard::Removed(bot) => self.bot_removed(&BotKey { home: home.clone(), id: bot }, cx),
            Heard::Nothing => {}
        }
    }

    fn line_heard(&mut self, home: &Home, bot: &str, line: bots::Line, cx: &mut Context<Self>) {
        match self.bots_at_mut(home).and_then(|bots| bots.heard(bot, line)) {
            Some(bots::Added::Yes) => cx.notify(),
            Some(bots::Added::Gap(last)) => self.read_chat(Some(last), cx),
            Some(bots::Added::No) | None => {}
        }
        let chat = self.bots_at(home).and_then(|bots| bots.chat.as_ref());
        if chat.is_some_and(|chat| chat.bot == bot && chat.failed()) {
            self.catch_up_chat(cx);
        }
        self.mind_seen(cx);
    }

    fn bot_removed(&mut self, key: &BotKey, cx: &mut Context<Self>) {
        let shown = self.shown_bot() == Some(key);
        let label = self.bot_label(key);
        let draft = self.bots_at_mut(&key.home).and_then(|bots| {
            bots.answers.forget_bot(&key.id);
            bots.drafts.take(&key.id)
        });
        self.drop_bot_toasts(key, cx);
        self.place(Moved::Removed(key.clone()));
        if self.bots_at(&key.home).is_some_and(|bots| bots.manage.told(&key.id, shown)) {
            let copy = talk::copy_action(&draft.unwrap_or_default());
            self.toast_with(format!("{label} was removed"), ToastKind::Info, copy, cx);
        }
        self.close_dialog_naming(key, cx);
        cx.notify();
    }

    pub(crate) fn bot_stream_started(&mut self, cx: &mut Context<Self>) {
        self.events_started(&Home::Here, cx);
    }

    fn events_started(&mut self, home: &Home, cx: &mut Context<Self>) {
        if self.shown_bot().is_some_and(|key| key.home == *home) {
            self.catch_up_chat(cx);
        }
    }

    fn catch_up_chat(&mut self, cx: &mut Context<Self>) {
        match self.shown_chat().map(|(_, chat)| chat.catch_up()) {
            Some(CatchUp::After(last)) => self.read_chat(Some(last), cx),
            Some(CatchUp::Whole) => self.read_chat(None, cx),
            Some(CatchUp::Nothing) | None => {}
        }
    }

    pub(crate) fn bots_window_active(&mut self, active: bool, cx: &mut Context<Self>) {
        if self.bots.active != active {
            self.bots.active = active;
            self.there.bots.set_active(active);
            self.mind_seen(cx);
        }
    }

    fn read_chat(&mut self, after: Option<u64>, cx: &mut Context<Self>) {
        let Some(key) = self.shown_bot().cloned() else { return };
        let busy = format!("bot-history:{}", key.tag());
        let deferred = after.filter(|_| self.busy(&busy));
        let unreachable = key.home.machine().and_then(|machine| self.bot_target(machine).err());
        let Some(bots) = self.bots_at_mut(&key.home) else { return };
        let Some(chat) = bots.chat.as_mut().filter(|chat| chat.bot == key.id) else { return };
        if let Some(why) = unreachable {
            chat.fail(talk::refused_words(false, &why));
            return cx.notify();
        }
        if let Some(from) = deferred {
            chat.defer(from);
            return;
        }
        let asked = if after.is_none() { bots.next_ask() } else { chat.asked };
        if let Some(chat) = bots.chat.as_mut().filter(|_| after.is_none()) {
            chat.asked = asked;
            chat.read = Read::Asking;
        }
        let busy = Busy::new(busy, format!("Reading the chat of {}…", self.bot_label(&key)));
        let call = Call::history(&key, after);
        self.bot_read(busy, call, Box::new(move |this, result, cx| this.chat_read((&key, asked), result, cx)), cx);
    }

    fn chat_read(&mut self, (key, asked): (&BotKey, u64), result: anyhow::Result<String>, cx: &mut Context<Self>) {
        let label = self.bot_label(key);
        let Some(bots) = self.bots_at_mut(&key.home) else { return };
        let Some(chat) = bots.chat.as_mut().filter(|chat| chat.awaits(&key.id, asked)) else { return };
        match result.and_then(|out| bots::history_of(&out)) {
            Ok(lines) => chat.take(lines),
            Err(error) => {
                let reason = cli::reason(&error);
                chat.fail(reason.clone());
                self.toast(format!("Could not read the chat of {label}: {reason}"), ToastKind::Error, cx);
            }
        }
        let pending = self.bots_at_mut(&key.home).and_then(|bots| bots.chat.as_mut().and_then(Chat::take_pending));
        if let Some(from) = pending {
            self.read_chat(Some(from), cx);
        }
        self.mind_seen(cx);
        cx.notify();
    }

    pub(super) fn mind_seen(&mut self, cx: &mut Context<Self>) {
        let Some(key) = self.shown_bot().cloned() else { return };
        let Some(bots) = self.bots_at(&key.home) else { return };
        let Some(id) = bots.seen_due(Some(&key.id)) else { return };
        let unread = bots.find(&id).map_or(0, |bot| bot.unread);
        let home = key.home.clone();
        let forget: talk::Then = Box::new(move |this, _, _| {
            if let Some(bots) = this.bots_at_mut(&home) {
                bots.forget_seen();
            }
        });
        if !self.bot_mark(Call::seen(&key), forget, cx) {
            return;
        }
        if let Some(bots) = self.bots_at_mut(&key.home) {
            bots.seen = Some((id, unread));
        }
        self.drop_bot_toasts(&key, cx);
    }
}
