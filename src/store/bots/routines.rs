use gpui::Context;

use super::super::{Busy, Store, ToastKind, args};
use super::talk::{Call, Then};
use crate::cli::{self, Feature, Has};
use crate::model::bots::{Bot, Home};
use crate::panels::PanelKind;
use crate::ui::Icon;

pub(crate) const GONE_RUN: &str = "That run is no longer in the queue";

pub(crate) fn closes(shown: Option<PanelKind>, asked: PanelKind, scoped: bool) -> bool {
    shown == Some(asked) && !(asked == PanelKind::Queue && scoped)
}

pub(crate) fn routines_title(name: &str) -> String {
    format!("Routines › {name}")
}

fn again_key(run: u64) -> String {
    format!("bot-run-again:{run}")
}

impl Store {
    pub(crate) fn routines_offered(&self) -> bool {
        self.routines_offered_at(&Home::Here)
    }

    pub(crate) fn routines_offered_at(&self, home: &Home) -> bool {
        Feature::BotRoutines.on(self.bot_releases(home).server) == Has::Yes
    }

    pub(crate) fn open_routines(&mut self, bot: &str, cx: &mut Context<Self>) {
        if self.bots.find(bot).is_none() {
            return;
        }
        self.open_sheet(PanelKind::Queue, cx);
        self.bots.queue_of = Some(bot.to_string());
    }

    pub(crate) fn routines_of(&self) -> Option<&Bot> {
        self.bots.queue_of.as_deref().and_then(|id| self.bots.find(id))
    }

    pub(crate) fn sheet_title(&self, kind: PanelKind) -> (Icon, String) {
        match self.routines_of().filter(|_| kind == PanelKind::Queue) {
            Some(bot) => (Icon::Repeat, routines_title(&bot.name)),
            None => (kind.icon(), kind.title().to_string()),
        }
    }

    pub(crate) fn run_again(&mut self, run: u64, cx: &mut Context<Self>) {
        let Some(target) = self.bots.runs.again(run) else {
            self.toast(GONE_RUN, ToastKind::Info, cx);
            return;
        };
        let label = self.bots.runs.label(run).unwrap_or_default().to_string();
        self.bots.again_asked(run);
        cx.notify();
        let busy = Busy::new(again_key(run), format!("Starting a run of “{label}”…"));
        let then: Then = Box::new(move |this, result, cx| {
            this.bots.again_ended(run);
            if let Err(error) = result {
                this.toast(cli::reason(&error), ToastKind::Error, cx);
            }
            cx.notify();
        });
        self.bot_job(busy, Call::here(args(&["queue", "run", &target.to_string()])), then, cx);
    }

    pub(super) fn close_routines_if_gone(&mut self, cx: &mut Context<Self>) {
        let gone = self.bots.queue_of.as_deref().is_some_and(|id| self.bots.find(id).is_none());
        if gone && self.sheet == Some(PanelKind::Queue) {
            self.close_sheet(cx);
        }
    }
}

#[cfg(test)]
mod tests;
