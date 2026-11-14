use std::time::Duration;

use gpui::{AppContext as _, Context};

use super::jobs::{folding_key, renaming_key};
use super::machine_read::{Listing, Reach, reach};
use super::{Busy, Store, ToastKind, run_job_on, run_with};
use crate::cli::{self, Target, Version};
use crate::model::machines::hide_destination;
use crate::model::machines::run_on::server_release;
use crate::model::machines::seen::{ReadReleases, sighted, sighted_pane};
use crate::model::tunnels::Release;
use crate::model::{Aim, MachinePlace, MachineSeen, PaneAim, ThereId};

const NOTHING_SENT: &str = "nothing was sent";
const FOLD_CAP: Duration = Duration::from_secs(15);

#[derive(Default)]
pub(super) struct CliRelease {
    known: Option<Version>,
    heard: bool,
    asks: u64,
}

impl CliRelease {
    fn ask(&mut self) -> u64 {
        self.asks += 1;
        self.asks
    }

    fn answered(&mut self, ask: u64, release: Option<Version>) -> bool {
        if ask != self.asks {
            return false;
        }
        let changed = self.known != release || !self.heard;
        (self.known, self.heard) = (release, true);
        changed
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NoticeThere {
    pub machine: String,
    pub boot: String,
    pub panes: Vec<ThereId>,
    pub gone: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MachineAct {
    Rename(String),
    Close,
}

impl MachineAct {
    fn args(&self, aim: &Aim) -> Vec<String> {
        match self {
            Self::Rename(name) => aim.rename_args(name),
            Self::Close => aim.close_args(),
        }
    }

    fn leaves(&self) -> bool {
        !matches!(self, Self::Rename(_))
    }

    fn job(&self, aim: &Aim, name: &str) -> Busy {
        let machine = &aim.machine;
        match self {
            Self::Rename(_) => Busy::new(renaming_key(aim), format!("Renaming {name} on {machine}…")),
            Self::Close => Busy::leaving(aim, format!("Closing {name} on {machine}…")),
        }
    }

    fn done(&self, aim: &Aim, name: &str) -> Option<String> {
        let machine = &aim.machine;
        match self {
            Self::Rename(_) => None,
            Self::Close => Some(format!("Closed {name} on {machine}")),
        }
    }
}

struct Acting {
    aim: Aim,
    act: MachineAct,
    name: String,
}

pub(crate) fn machine_target(window: &Target, listing: Listing<'_>, seen: &MachineSeen) -> Result<Target, String> {
    match reach(window, listing, seen) {
        Reach::Up(target) => Ok(target),
        other => Err(other.why_not().unwrap_or_default().to_string()),
    }
}

fn sent_or_not(why: &str) -> String {
    format!("{why}: {NOTHING_SENT}")
}

pub(crate) fn restart_note(machine: &str) -> String {
    format!("{machine} restarted")
}

pub(crate) fn read_refusal(machine: &str, error: &anyhow::Error) -> Option<String> {
    cli::stale_boot(error).then(|| restart_note(machine))
}

pub(crate) fn stale_note(machine: &str, error: &anyhow::Error) -> Option<String> {
    read_refusal(machine, error).map(|why| sent_or_not(&why))
}

pub(crate) fn part_sent_note(machine: &str) -> String {
    format!("{machine} restarted after part was sent: hand it again")
}

pub(crate) fn answer_words(queue: &cli::Queue, answer: Option<anyhow::Result<String>>) -> Option<String> {
    answer.and_then(Result::err).map(|error| queue_words(queue, &error))
}

pub(crate) fn queue_words(queue: &cli::Queue, error: &anyhow::Error) -> String {
    queue.machine().map_or_else(|| error.to_string(), |machine| failed_there(machine, error))
}

pub(crate) fn failed_there(machine: &str, error: &anyhow::Error) -> String {
    stale_note(machine, error).unwrap_or_else(|| format!("{machine}: {}", cli::reason(error)))
}

fn fold_refusal(aim: &PaneAim, destination: &str, error: &anyhow::Error) -> String {
    stale_note(&aim.machine, error)
        .unwrap_or_else(|| aim.failed(&hide_destination(&cli::reason(error), &aim.machine, destination)))
}

impl Store {
    pub(crate) fn pick_machine_place(&mut self, place: MachinePlace, cx: &mut Context<Self>) {
        self.placing.awaited = None;
        self.select_there(place, cx);
    }

    pub(super) fn select_there(&mut self, place: MachinePlace, cx: &mut Context<Self>) {
        let machine = place.machine.clone();
        self.recheck_there(&machine, cx);
        self.machine_place = Some(place);
        self.ask_machine_home(&machine, cx);
        self.file_full_screen = false;
        self.give_keys_back(cx);
    }

    pub(crate) fn leave_machine_place(&mut self, cx: &mut Context<Self>) {
        self.placing.awaited = None;
        if self.machine_place.take().is_some() {
            self.file_full_screen = false;
            cx.notify();
        }
    }

    pub(crate) fn follow_machines(&mut self, cx: &mut Context<Self>) {
        let Some(session) = &self.session else { return };
        let followed = self.machine_place.as_ref().and_then(|place| place.followed(&session.machines));
        let gone = self
            .machine_place
            .as_ref()
            .filter(|_| followed.is_none())
            .map(|place| (!self.machine_leaving(&place.aim())).then(|| place.gone_note()));
        self.machine_place = followed;
        if let Some(session) = &self.session {
            self.jobs.keep_leaving_there(|aim| sighted(&session.machines, aim).is_ok());
        }
        if let Some(text) = gone.flatten() {
            self.toast(text, ToastKind::Info, cx);
        }
    }

    fn place_of_notice(&self, there: &NoticeThere) -> Option<MachinePlace> {
        self.session.as_ref()?.place_holding_panes((&there.machine, &there.boot), &there.panes)
    }

    pub(crate) fn go_there(&mut self, there: &NoticeThere, cx: &mut Context<Self>) {
        match self.place_of_notice(there) {
            Some(place) => self.pick_machine_place(place, cx),
            None => self.toast(there.gone.clone(), ToastKind::Info, cx),
        }
    }

    pub(crate) fn shows_notice(&self, there: &NoticeThere) -> bool {
        let shown = self.machine_place.as_ref();
        shown.zip(self.place_of_notice(there)).is_some_and(|(shown, place)| shown.same_workspace(&place))
    }

    pub(crate) fn machine_target(&self, seen: &MachineSeen) -> Result<Target, String> {
        machine_target(&cli::target(), self.listing(), seen)
    }

    pub(crate) fn reach_machine(&self, aim: &Aim) -> Result<Target, String> {
        let machines = self.session.as_ref().map_or(&[][..], |session| &session.machines);
        let (seen, ws) = sighted(machines, aim)?;
        let target = self.machine_target(seen)?;
        if self.machine_busy(aim) {
            return Err(format!("{} on {} is still waiting for its last action", ws.name, seen.name));
        }
        Ok(target)
    }

    pub(crate) fn machine_busy(&self, aim: &Aim) -> bool {
        self.machine_leaving(aim) || self.jobs.busy(&renaming_key(aim))
    }

    pub(crate) fn machine_leaving(&self, aim: &Aim) -> bool {
        self.jobs.leaving_there(aim)
    }

    pub(crate) fn worktree_went_there(&mut self, aim: Aim, cx: &mut Context<Self>) {
        self.jobs.left_there(aim);
        cx.notify();
    }

    pub(crate) fn not_sent(&mut self, why: &str, cx: &mut Context<Self>) {
        self.toast(sent_or_not(why), ToastKind::Error, cx);
    }

    pub(crate) fn act_on_machine(&mut self, aim: Aim, act: MachineAct, cx: &mut Context<Self>) {
        let target = match self.reach_machine(&aim) {
            Ok(target) => target,
            Err(why) => return self.not_sent(&why, cx),
        };
        let machines = self.session.as_ref().map_or(&[][..], |session| &session.machines);
        let name = sighted(machines, &aim).map(|(_, ws)| ws.name.clone()).unwrap_or_default();
        let id = self.start_job(act.job(&aim, &name), cx);
        let command = act.args(&aim);
        let acting = Acting { aim, act, name };
        run_job_on(cx, id, target, command, move |this: &mut Self, result, cx| this.acted(acting, result.err(), cx));
    }

    fn acted(&mut self, acting: Acting, failed: Option<anyhow::Error>, cx: &mut Context<Self>) {
        let Acting { aim, act, name } = acting;
        if let Some(e) = failed {
            let text = failed_there(&aim.machine, &e);
            self.toast(text, ToastKind::Error, cx);
            return;
        }
        if let Some(done) = act.done(&aim, &name) {
            self.toast(done, ToastKind::Info, cx);
        }
        if act.leaves() {
            self.jobs.left_there(aim);
        }
        cx.notify();
    }

    pub(crate) fn reach_pane(&self, aim: &PaneAim) -> Result<Target, String> {
        let machines = self.session.as_ref().map_or(&[][..], |session| &session.machines);
        self.machine_target(sighted_pane(machines, aim)?)
    }

    pub(crate) fn folding_on_machine(&self, aim: &PaneAim) -> bool {
        self.jobs.busy(&folding_key(aim))
    }

    pub(crate) fn fold_on_machine(&mut self, aim: PaneAim, cx: &mut Context<Self>) {
        let target = match self.reach_pane(&aim) {
            Ok(target) => target,
            Err(why) => return self.not_sent(&why, cx),
        };
        let job = (Store::get(cx), self.start_job(Busy::new(folding_key(&aim), aim.running()), cx));
        let command = aim.args(self.cli_release());
        let destination = target.host().unwrap_or_default().to_string();
        let answer = move || cli::run_on_within(&target, &command, FOLD_CAP);
        run_with(cx, Some(job), answer, move |this: &mut Self, result, cx| {
            if let Err(e) = result {
                this.toast(fold_refusal(&aim, &destination, &e), ToastKind::Error, cx);
            }
        });
    }

    pub(crate) fn cli_release(&self) -> Option<Version> {
        self.there.release.known
    }

    pub(crate) fn read_releases(&self, machine: &str) -> ReadReleases {
        ReadReleases {
            here: self.cli_release(),
            there: server_release(Some(machine), self.session.as_ref(), &self.machines),
        }
    }

    pub(crate) fn heard_release(&self) -> Release {
        if self.there.release.heard { Release::Read(self.there.release.known) } else { Release::Unread }
    }

    pub(crate) fn read_cli_release(&mut self, cx: &mut Context<Self>) {
        let ask = self.there.release.ask();
        let read = cx.background_spawn(async { cli::installed_version() });
        cx.spawn(async move |this, cx| {
            let release = read.await;
            let _ = this.update(cx, |this, cx| this.heard_cli_release(ask, release, cx));
        })
        .detach();
    }

    fn heard_cli_release(&mut self, ask: u64, release: Option<Version>, cx: &mut Context<Self>) {
        if self.there.release.answered(ask, release) {
            if self.view_id.is_some() {
                self.follow_tunnels(cx);
            }
            cx.notify();
        }
    }
}

#[cfg(test)]
mod tests;
