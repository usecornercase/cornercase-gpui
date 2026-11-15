use gpui::Context;

use super::super::{Busy, Clip, Store, ToastAction, ToastGo, ToastKind, reply_of};
use crate::cli;
use crate::model::bots::{Goes, Said, refused_goes, reset_key, say_args, say_key, sendable, stop_key};

pub(crate) const COPY_MESSAGE: &str = "Copy message";

pub(super) fn copy_action(text: &str) -> ToastAction {
    sendable(text).then(|| {
        (COPY_MESSAGE.to_string(), ToastGo::Copy(Clip { text: text.to_string(), said: "Copied your message".into() }))
    })
}

pub(super) type Then = Box<dyn FnOnce(&mut Store, anyhow::Result<String>, &mut Context<Store>)>;

pub(super) struct Call {
    args: Vec<String>,
    stdin: Option<Vec<u8>>,
}

impl Call {
    fn say(on_its_way: bool, bot: &str, text: &str) -> Option<Self> {
        (!on_its_way).then(|| Self { args: say_args(bot), stdin: Some(text.as_bytes().to_vec()) })
    }

    pub(super) fn args(args: Vec<String>) -> Self {
        Self { args, stdin: None }
    }

    pub(super) fn of(words: &[&str]) -> Self {
        Self { args: words.iter().map(|word| (*word).to_string()).collect(), stdin: None }
    }

    fn stop(bot: &str) -> Self {
        Self::of(&["bot", "stop", bot])
    }

    fn reset(bot: &str) -> Self {
        Self::of(&["bot", "reset", bot])
    }
}

struct Sent {
    bot: String,
    name: String,
    text: String,
    said: Said,
}

impl Store {
    pub(super) fn bot_name(&self, bot: &str) -> String {
        self.bots.find(bot).map_or_else(|| bot.to_string(), |found| found.name.clone())
    }

    pub(super) fn bot_job(&mut self, busy: Busy, call: Call, then: Then, cx: &mut Context<Self>) {
        let id = self.start_job(busy, cx);
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

    pub(crate) fn say_to_bot(&mut self, bot: &str, text: String, said: Said, cx: &mut Context<Self>) {
        let Some(call) = Call::say(self.busy(&say_key(bot)), bot, &text) else { return };
        let name = self.bot_name(bot);
        let busy = Busy::new(say_key(bot), format!("Sending to {name}…"));
        let sent = Sent { bot: bot.to_string(), name, text, said };
        let then: Then = Box::new(move |this, result, cx| {
            if let Err(error) = result {
                this.said_failed(&sent, &error, cx);
            }
        });
        self.bot_job(busy, call, then, cx);
    }

    fn said_failed(&mut self, sent: &Sent, error: &anyhow::Error, cx: &mut Context<Self>) {
        let words = format!("Could not send to {}: {}", sent.name, cli::reason(error));
        match refused_goes(sent.said, self.bots.find(&sent.bot).is_some()) {
            Goes::Draft => {
                self.bots.drafts.put_back(&sent.bot, &sent.text);
                self.toast(words, ToastKind::Error, cx);
            }
            Goes::Toast => self.toast_with(words, ToastKind::Error, copy_action(&sent.text), cx),
            Goes::Nowhere => self.toast(words, ToastKind::Error, cx),
        }
        cx.notify();
    }

    pub(crate) fn keep_draft(&mut self, bot: &str, text: String) {
        self.bots.drafts.keep(bot, text);
    }

    pub(crate) fn add_example(&mut self, bot: &str, example: &str, cx: &mut Context<Self>) {
        self.bots.drafts.add_example(bot, example);
        cx.notify();
    }

    pub(crate) fn stop_bot(&mut self, bot: &str, cx: &mut Context<Self>) {
        if self.bots.stopping(bot) {
            return;
        }
        self.bots.stop_asked(bot);
        let (name, id) = (self.bot_name(bot), bot.to_string());
        let busy = Busy::new(stop_key(bot), format!("Stopping {name}…"));
        let then: Then = Box::new(move |this, result, cx| {
            if let Err(error) = result {
                this.bots.stop_failed(&id);
                this.toast(format!("Could not stop {name}: {}", cli::reason(&error)), ToastKind::Error, cx);
            }
            cx.notify();
        });
        self.bot_job(busy, Call::stop(bot), then, cx);
        cx.notify();
    }

    pub(crate) fn new_conversation(&mut self, bot: &str, cx: &mut Context<Self>) {
        let name = self.bot_name(bot);
        let busy = Busy::new(reset_key(bot), format!("Starting a new conversation with {name}…"));
        let then: Then = Box::new(move |this, result, cx| {
            if let Err(error) = result {
                let words = format!("Could not start a new conversation with {name}: {}", cli::reason(&error));
                this.toast(words, ToastKind::Error, cx);
            }
        });
        self.bot_job(busy, Call::reset(bot), then, cx);
    }
}

#[cfg(test)]
mod tests;
