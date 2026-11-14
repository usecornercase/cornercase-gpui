use std::time::{Duration, Instant};

use gpui::Context;

use crate::cli::{self, Stream, StreamEnd, Target};
use crate::model::{ReadReleases, Session};
use crate::panels::PanelKind;
use crate::panels::on_machine::{self, Retry};
use crate::store::{self, MachineRead, Store, args};

use super::pieces::find;
use super::wire::{self, State, Summary};
use super::{ChangesPanel, Follow, Scope, Whose};

#[derive(Debug, Default)]
pub(super) struct Hearing {
    on: Option<Target>,
    pub(super) retry: Retry,
    boot_heard: bool,
}

impl Hearing {
    fn start_over(&mut self) {
        self.retry = Retry::default();
    }

    fn opened(&mut self, on: Option<Target>) {
        self.on = on;
        self.boot_heard = false;
    }

    pub(super) fn read_refused(&mut self, error: &anyhow::Error) -> bool {
        let refused = cli::stale_boot(error);
        if refused {
            self.refused();
        }
        refused
    }

    fn refused(&mut self) {
        self.boot_heard = false;
        self.retry.restarted();
    }

    fn ended(&mut self, why: String, now: Instant) -> Duration {
        self.boot_heard = false;
        self.retry.failed(why, now)
    }

    fn believed(&mut self) -> bool {
        self.boot_heard = true;
        self.retry.heard()
    }

    pub(super) fn may_read(&self, read: &MachineRead) -> Result<Target, String> {
        let target = on_machine::may_ask(read, &self.retry)?;
        if self.boot_heard { Ok(target) } else { Err(format!("{} has not answered yet", read.aim.machine)) }
    }
}

fn list_args(follow: &Follow, releases: ReadReleases) -> Vec<String> {
    let mut a = args(&["changes", "list", "--follow"]);
    let Some(whose) = &follow.workspace else {
        a.push("--every".into());
        return a;
    };
    a.extend(["--workspace", &whose.arg(), "--mode", follow.scope.mode().name()].map(String::from));
    whose.reading(a, releases)
}

#[derive(Debug, PartialEq, Eq)]
enum AfterEnd {
    Refused,
    TryAgain(String),
    Ended(String),
}

fn after_end(machine: Option<&str>, ended: &StreamEnd) -> AfterEnd {
    let why = ended_reason(ended.said());
    match machine {
        Some(_) if ended.stale_boot() => AfterEnd::Refused,
        Some(_) => AfterEnd::TryAgain(why),
        None => AfterEnd::Ended(why),
    }
}

fn ended_reason(err: &str) -> String {
    let err = err.trim();
    if err.is_empty() { "the changes stopped".to_string() } else { err.to_string() }
}

pub(super) fn shown_whose(session: Option<&Session>, read: Option<&MachineRead>) -> Option<Whose> {
    match read {
        Some(read) => Some(Whose::There(read.aim.clone(), read.destination.clone())),
        None => session?.shown().workspace.map(Whose::Here),
    }
}

pub(super) fn stream_target(follow: Option<&Follow>, read: Option<&MachineRead>, window: &Target) -> Option<Target> {
    match follow?.workspace {
        Some(Whose::There(..)) => read?.reach.target().cloned(),
        _ => Some(window.clone()),
    }
}

pub(super) fn whose_listed(follow: Option<&Follow>, listed: u64) -> Whose {
    match follow {
        Some(Follow { workspace: Some(there @ Whose::There(..)), .. }) => there.clone(),
        _ => Whose::Here(listed),
    }
}

fn from_another_server(summary: &Summary, follow: Option<&Follow>) -> bool {
    let Some(Follow { workspace: Some(Whose::There(aim, _)), .. }) = follow else { return false };
    !summary.boot.is_empty() && summary.boot != aim.boot
}

fn only_starting(summary: &Summary) -> bool {
    summary.state == State::Loading && summary.workspaces.iter().all(|w| w.files.is_empty())
}

impl ChangesPanel {
    pub(super) fn sync(&mut self, cx: &mut Context<Self>) {
        let store = Store::get(cx);
        let s = store.read(cx);
        let docked = s.dock == Some(PanelKind::Changes);
        let read = s.machine_read();
        let shown = shown_whose(s.session.as_ref(), read.as_ref());
        if read != self.on_machine {
            self.on_machine = read;
            cx.notify();
        }
        if shown != self.shown {
            self.shown.clone_from(&shown);
            self.base = None;
            self.base_open = false;
            cx.notify();
            if matches!(self.scope, Scope::Project(_)) {
                self.select(None, cx);
            }
        }
        let want = if docked { self.wanted_follow(shown) } else { None };
        if want != self.follow {
            self.restart(want, cx);
        } else if self.wanted_target() != self.hearing.on {
            self.hearing.start_over();
            self.open_stream(cx);
        }
    }

    fn wanted_follow(&self, shown: Option<Whose>) -> Option<Follow> {
        match self.scope {
            Scope::Project(_) => shown.map(|w| Follow { scope: self.scope, workspace: Some(w) }),
            Scope::Every => Some(Follow { scope: Scope::Every, workspace: None }),
        }
    }

    fn wanted_target(&self) -> Option<Target> {
        stream_target(self.follow.as_ref(), self.on_machine.as_ref(), &cli::target())
    }

    fn restart(&mut self, follow: Option<Follow>, cx: &mut Context<Self>) {
        self.follow = follow;
        self.summary = None;
        self.ended = None;
        self.hearing.start_over();
        self.open_stream(cx);
    }

    fn open_stream(&mut self, cx: &mut Context<Self>) {
        self.stream = None;
        self.stream_generation += 1;
        self.hearing.opened(self.wanted_target());
        self.stream = self.follow_now(cx);
        cx.notify();
    }

    fn follow_now(&self, cx: &mut Context<Self>) -> Option<Stream> {
        let (follow, target) = (self.follow.as_ref()?, self.hearing.on.as_ref()?);
        let generation = self.stream_generation;
        let releases =
            follow.workspace.as_ref().map(|whose| whose.releases(Store::get(cx).read(cx))).unwrap_or_default();
        store::follow_on(
            cx,
            target,
            list_args(follow, releases),
            move |this, line, cx| this.on_line(generation, &line, cx),
            move |this, ended, cx| this.on_end(generation, &ended, cx),
        )
    }

    fn on_line(&mut self, generation: u64, line: &str, cx: &mut Context<Self>) {
        if self.stream_generation == generation {
            self.on_summary(line, cx);
        }
    }

    pub(super) fn machine_followed(&self) -> Option<String> {
        let Some(Follow { workspace: Some(Whose::There(aim, _)), .. }) = &self.follow else { return None };
        Some(aim.machine.clone())
    }

    fn on_end(&mut self, generation: u64, ended: &StreamEnd, cx: &mut Context<Self>) {
        if self.stream_generation != generation {
            return;
        }
        self.stream = None;
        let machine = self.machine_followed();
        match after_end(machine.as_deref(), ended) {
            AfterEnd::Refused => self.hearing.refused(),
            AfterEnd::TryAgain(why) => {
                let machine = machine.unwrap_or_default();
                Store::get(cx).update(cx, |s, cx| s.there_failed(&machine, cx));
                self.try_again_later(why, cx);
            }
            AfterEnd::Ended(why) => self.ended = Some(why),
        }
        cx.notify();
    }

    fn try_again_later(&mut self, why: String, cx: &mut Context<Self>) {
        let generation = self.stream_generation;
        let wait = self.hearing.ended(why, Instant::now());
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            let _ = this.update(cx, |this, cx| this.reopen(generation, cx));
        })
        .detach();
    }

    fn reopen(&mut self, generation: u64, cx: &mut Context<Self>) {
        if self.stream_generation == generation {
            self.open_stream(cx);
        }
    }

    fn on_summary(&mut self, line: &str, cx: &mut Context<Self>) {
        let Ok(summary) = wire::summary(line) else { return };
        if from_another_server(&summary, self.follow.as_ref()) {
            self.server_moved_on(cx);
            return;
        }
        let heard = self.hearing.believed();
        if self.reads_machine() && self.summary.is_some() && only_starting(&summary) {
            if heard {
                cx.notify();
            }
            return;
        }
        self.summary = Some(summary);
        self.refresh_open(cx);
        cx.notify();
    }

    fn server_moved_on(&mut self, cx: &mut Context<Self>) {
        self.stream = None;
        self.stream_generation += 1;
        self.open.drop_reads_under_way();
        let machine = self.on_machine.as_ref().map_or("the machine", |read| read.aim.machine.as_str());
        self.try_again_later(format!("{machine} restarted"), cx);
        cx.notify();
    }

    fn refresh_open(&mut self, cx: &mut Context<Self>) {
        let Some(selected) = self.selected.clone() else { return };
        let Some(summary) = &self.summary else { return };
        if summary.state != State::Ready {
            return;
        }
        match find(summary, &selected.workspace, &selected.path) {
            Some(file) if file.digest != self.open.digest => {
                let digest = file.digest.clone();
                self.load_patch(digest, cx);
            }
            Some(_) => {}
            None => self.select(None, cx),
        }
    }
}

#[cfg(test)]
mod tests;
