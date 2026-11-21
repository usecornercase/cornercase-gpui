use gpui::Context;

use super::super::machine_read::Facts;
use super::super::machines_there::sent_or_not;
use super::super::{Busy, Clip, Store, ToastAction, ToastGo, ToastKind, args, reply_of, run_job, run_with};
use crate::cli::{self, Target};
use crate::model::bots::{
    BotKey, Goes, Home, Said, refused_goes, reset_key, say_args, say_key, sendable, setup_args, stop_key,
};
use crate::model::machines::hide_destination;

pub(crate) const COPY_MESSAGE: &str = "Copy message";

pub(super) fn copy_action(text: &str) -> ToastAction {
    sendable(text).then(|| {
        (COPY_MESSAGE.to_string(), ToastGo::Copy(Clip { text: text.to_string(), said: "Copied your message".into() }))
    })
}

pub(super) type Then = Box<dyn FnOnce(&mut Store, anyhow::Result<String>, &mut Context<Store>)>;

pub(super) struct Call {
    home: Home,
    args: Vec<String>,
    stdin: Option<Vec<u8>>,
    sends: bool,
}

pub(super) fn refused_words(sends: bool, why: &str) -> String {
    if sends { sent_or_not(why) } else { why.to_string() }
}

impl Call {
    fn to(key: &BotKey, args: Vec<String>) -> Self {
        Self { home: key.home.clone(), args, stdin: None, sends: true }
    }

    fn say(on_its_way: bool, key: &BotKey, text: &str) -> Option<Self> {
        (!on_its_way).then(|| Self { stdin: Some(text.as_bytes().to_vec()), ..Self::to(key, say_args(&key.id)) })
    }

    fn setup(on_its_way: bool, key: &BotKey) -> Option<Self> {
        (!on_its_way).then(|| Self::to(key, setup_args(&key.id)))
    }

    pub(super) fn here(args: Vec<String>) -> Self {
        Self { home: Home::Here, args, stdin: None, sends: true }
    }

    pub(super) fn here_with_stdin(args: Vec<String>, stdin: Vec<u8>) -> Self {
        Self { stdin: Some(stdin), ..Self::here(args) }
    }

    fn stop(key: &BotKey) -> Self {
        Self::to(key, args(&["bot", "stop", &key.id]))
    }

    fn reset(key: &BotKey) -> Self {
        Self::to(key, args(&["bot", "reset", &key.id]))
    }

    pub(super) fn history(key: &BotKey, after: Option<u64>) -> Self {
        let from = after.map(|last| last.to_string());
        let mut words = vec!["bot", "history", &key.id, "--json"];
        match from.as_deref() {
            Some(last) => words.extend(["--after", last]),
            None => words.extend(["--last", "1000"]),
        }
        Self { sends: false, ..Self::to(key, args(&words)) }
    }

    pub(super) fn seen(key: &BotKey) -> Self {
        Self { sends: false, ..Self::to(key, args(&["bot", "seen", &key.id])) }
    }
}

struct Sent {
    bot: BotKey,
    name: String,
    text: String,
    said: Said,
}

fn hidden(error: &anyhow::Error, machine: &str, target: &Target) -> anyhow::Error {
    let destination = match target {
        Target::Remote(remote) => remote.destination(),
        Target::Local => "",
    };
    anyhow::anyhow!(hide_destination(&cli::reason(error), machine, destination))
}

fn refuse_later(then: Then, why: String, cx: &mut Context<Store>) {
    cx.spawn(async move |this, cx| {
        let _ = this.update(cx, |this, cx| then(this, Err(anyhow::anyhow!(why)), cx));
    })
    .detach();
}

impl Store {
    pub(super) fn bot_target(&self, machine: &str) -> Result<Target, String> {
        let Some(seen) = self.session.as_ref().and_then(|session| session.machine(machine)) else {
            return Err(format!("{machine} is no longer listed"));
        };
        self.reach_seen(seen).target().cloned().ok_or_else(|| {
            self.reach_seen(seen).why_not().map_or_else(|| format!("{machine} cannot be reached"), str::to_string)
        })
    }

    pub(super) fn bot_job(&mut self, busy: Busy, call: Call, then: Then, cx: &mut Context<Self>) {
        self.bot_run((busy, call), then, true, cx);
    }

    pub(super) fn bot_read(&mut self, busy: Busy, call: Call, then: Then, cx: &mut Context<Self>) {
        self.bot_run((busy, call), then, false, cx);
    }

    fn bot_run(&mut self, (busy, call): (Busy, Call), then: Then, queued: bool, cx: &mut Context<Self>) {
        if let Home::Machine(machine) = &call.home {
            return self.machine_job(&machine.clone(), (busy, call), then, cx);
        }
        let id = self.start_job(busy, cx);
        if !queued {
            return run_job(cx, id, call.args, then);
        }
        let rx = reply_of(&self.queue, call.args, call.stdin);
        cx.spawn(async move |this, cx| {
            let result = rx.await.unwrap_or_else(|_| Err(anyhow::anyhow!("no answer came")));
            let _ = this.update(cx, |this, cx| {
                this.end_job(id, cx);
                then(this, result, cx);
            });
        })
        .detach();
    }

    pub(super) fn bot_mark(&mut self, call: Call, failed: Then, cx: &mut Context<Self>) -> bool {
        let Home::Machine(machine) = &call.home else {
            self.cmd(call.args);
            return true;
        };
        let Ok(target) = self.bot_target(machine) else { return false };
        run_with(
            cx,
            None,
            move || cli::run_on(&target, &call.args, None),
            move |this, result, cx| {
                if result.is_err() {
                    failed(this, result, cx);
                }
            },
        );
        true
    }

    fn machine_job(&mut self, machine: &str, (busy, call): (Busy, Call), then: Then, cx: &mut Context<Self>) {
        let target = match self.bot_target(machine) {
            Ok(target) => target,
            Err(why) => return refuse_later(then, refused_words(call.sends, &why), cx),
        };
        let (facts, machine) = (self.there_facts(machine), machine.to_string());
        let job = Some((cx.entity(), self.start_job(busy, cx)));
        let ran = target.clone();
        let work = move || cli::run_on(&ran, &call.args, call.stdin.as_deref());
        run_with(cx, job, work, move |this: &mut Self, result, cx| {
            let result = result.map_err(|error| this.there_error(&machine, (&error, facts), &target, cx));
            then(this, result, cx);
        });
    }

    fn there_error(
        &mut self,
        machine: &str,
        (error, facts): (&anyhow::Error, Option<Facts>),
        target: &Target,
        cx: &mut Context<Self>,
    ) -> anyhow::Error {
        if !cli::stale_boot(error) {
            self.there_refused(machine, (error, facts), cx);
        }
        hidden(error, machine, target)
    }

    pub(crate) fn say_to_bot(&mut self, bot: &BotKey, text: String, said: Said, cx: &mut Context<Self>) {
        let Some(call) = Call::say(self.busy(&say_key(bot)), bot, &text) else { return };
        let name = self.bot_label(bot);
        let busy = Busy::new(say_key(bot), format!("Sending to {name}…"));
        let sent = Sent { bot: bot.clone(), name, text, said };
        let then: Then = Box::new(move |this, result, cx| {
            if let Err(error) = result {
                this.said_failed(&sent, &error, cx);
            }
        });
        self.bot_job(busy, call, then, cx);
    }

    fn said_failed(&mut self, sent: &Sent, error: &anyhow::Error, cx: &mut Context<Self>) {
        let words = format!("Could not send to {}: {}", sent.name, cli::reason(error));
        match refused_goes(sent.said, self.bot(&sent.bot).is_some()) {
            Goes::Draft => {
                if let Some(bots) = self.bots_at_mut(&sent.bot.home) {
                    bots.drafts.put_back(&sent.bot.id, &sent.text);
                }
                self.toast(words, ToastKind::Error, cx);
            }
            Goes::Toast => self.toast_with(words, ToastKind::Error, copy_action(&sent.text), cx),
            Goes::Nowhere => self.toast(words, ToastKind::Error, cx),
        }
        cx.notify();
    }

    pub(crate) fn keep_draft(&mut self, bot: &BotKey, text: String) {
        if let Some(bots) = self.bots_at_mut(&bot.home) {
            bots.drafts.keep(&bot.id, text);
        }
    }

    pub(crate) fn add_example(&mut self, bot: &BotKey, example: &str, cx: &mut Context<Self>) {
        if let Some(bots) = self.bots_at_mut(&bot.home) {
            bots.drafts.add_example(&bot.id, example);
        }
        cx.notify();
    }

    pub(crate) fn stop_bot(&mut self, bot: &BotKey, cx: &mut Context<Self>) {
        let Some(bots) = self.bots_at_mut(&bot.home).filter(|bots| !bots.stopping(&bot.id)) else { return };
        bots.stop_asked(&bot.id);
        let (name, key) = (self.bot_label(bot), bot.clone());
        let busy = Busy::new(stop_key(bot), format!("Stopping {name}…"));
        let then: Then = Box::new(move |this, result, cx| {
            if let Err(error) = result {
                this.stop_failed(&key);
                this.toast(format!("Could not stop {name}: {}", cli::reason(&error)), ToastKind::Error, cx);
            }
            cx.notify();
        });
        self.bot_job(busy, Call::stop(bot), then, cx);
        cx.notify();
    }

    fn stop_failed(&mut self, key: &BotKey) {
        if let Some(bots) = self.bots_at_mut(&key.home) {
            bots.stop_failed(&key.id);
        }
    }

    pub(crate) fn new_conversation(&mut self, bot: &BotKey, cx: &mut Context<Self>) {
        let name = self.bot_label(bot);
        let busy = Busy::new(reset_key(bot), format!("Starting a new conversation with {name}…"));
        self.bot_act(busy, Call::reset(bot), format!("Could not start a new conversation with {name}"), cx);
    }

    pub(crate) fn set_up_bot(&mut self, bot: &BotKey, cx: &mut Context<Self>) {
        let Some(call) = Call::setup(self.busy(&say_key(bot)), bot) else { return };
        let name = self.bot_label(bot);
        let busy = Busy::new(say_key(bot), format!("Setting {name} up…"));
        self.bot_act(busy, call, format!("Could not set {name} up"), cx);
    }

    fn bot_act(&mut self, busy: Busy, call: Call, failed: String, cx: &mut Context<Self>) {
        let then: Then = Box::new(move |this, result, cx| {
            if let Err(error) = result {
                this.toast(format!("{failed}: {}", cli::reason(&error)), ToastKind::Error, cx);
            }
        });
        self.bot_job(busy, call, then, cx);
    }
}

#[cfg(test)]
mod tests;
