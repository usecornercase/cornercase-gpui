use std::collections::HashMap;
use std::time::Duration;

use gpui::Context;
use serde_json::Value;

use super::jobs::{folding_key, renaming_key};
use super::{Busy, Store, ToastKind, args, run_job_on, run_on, run_with};
use crate::cli::{self, Remote, Target};
use crate::desktop;
use crate::model::machines::seen::{sighted, sighted_pane};
use crate::model::machines::{Machine, hide_destination};
use crate::model::{Aim, MachinePlace, MachineSeen, PaneAim, ThereId};

const NOTHING_SENT: &str = "nothing was sent";
const FOLD_CAP: Duration = Duration::from_secs(15);

#[derive(Default)]
pub(super) struct Homes {
    known: HashMap<String, (String, String)>,
    asked: HashMap<String, String>,
}

impl Homes {
    fn ask(&mut self, machine: &str, boot: &str) -> bool {
        let known = self.known.get(machine).is_some_and(|(held, _)| held == boot);
        if known || self.asked.get(machine).is_some_and(|asked| asked == boot) {
            return false;
        }
        self.asked.insert(machine.to_owned(), boot.to_owned());
        true
    }

    fn answered(&mut self, machine: &str, boot: &str, home: Option<String>) {
        if self.asked.get(machine).is_none_or(|asked| asked != boot) {
            return;
        }
        self.asked.remove(machine);
        if let Some(home) = home {
            self.known.insert(machine.to_owned(), (boot.to_owned(), home));
        }
    }

    fn home(&self, machine: &str, boot: &str) -> Option<&str> {
        self.known.get(machine).filter(|(held, _)| held == boot).map(|(_, home)| home.as_str())
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

pub(crate) fn machine_target(window: &Target, list: &[Machine], seen: &MachineSeen) -> Result<Target, String> {
    let name = &seen.name;
    if let Some(host) = window.host() {
        return Err(format!("{name} is linked from {host}: its address means nothing from this {}", desktop::here()));
    }
    if !seen.linked() {
        return Err(seen.link_note());
    }
    let listed =
        list.iter().find(|machine| machine.name == *name).ok_or_else(|| format!("{name} is no longer listed"))?;
    let remote = Remote::new(&listed.destination, "");
    remote.map(Target::Remote).ok_or_else(|| format!("{name}'s ssh destination cannot be used"))
}

fn sent_or_not(why: &str) -> String {
    format!("{why}: {NOTHING_SENT}")
}

pub(crate) fn stale_note(machine: &str, error: &anyhow::Error) -> Option<String> {
    cli::stale_boot(error).then(|| sent_or_not(&format!("{machine} restarted")))
}

pub(crate) fn failed_there(machine: &str, error: &anyhow::Error) -> String {
    stale_note(machine, error).unwrap_or_else(|| format!("{machine}: {}", cli::reason(error)))
}

fn home_in(status: &str) -> Option<String> {
    let status: Value = serde_json::from_str(status).ok()?;
    status.get("home").and_then(Value::as_str).map(str::to_string)
}

impl Store {
    pub(crate) fn pick_machine_place(&mut self, place: MachinePlace, cx: &mut Context<Self>) {
        self.awaited = None;
        self.select_there(place, cx);
    }

    pub(super) fn select_there(&mut self, place: MachinePlace, cx: &mut Context<Self>) {
        self.ask_machine_home(&place.machine, cx);
        self.machine_place = Some(place);
        self.file_full_screen = false;
        self.give_keys_back(cx);
    }

    pub(crate) fn leave_machine_place(&mut self, cx: &mut Context<Self>) {
        self.awaited = None;
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
        machine_target(&cli::target(), &self.machines, seen)
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
        let command = aim.args();
        let destination = target.host().unwrap_or_default().to_string();
        let answer = move || cli::run_on_within(&target, &command, FOLD_CAP);
        run_with(cx, Some(job), answer, move |this: &mut Self, result, cx| {
            if let Err(e) = result {
                let reason = hide_destination(&cli::reason(&e), &aim.machine, &destination);
                this.toast(aim.failed(&reason), ToastKind::Error, cx);
            }
        });
    }

    pub(crate) fn machine_home(&self, machine: &str, boot: &str) -> Option<&str> {
        self.there.homes.home(machine, boot)
    }

    pub(crate) fn ask_machine_home(&mut self, machine: &str, cx: &mut Context<Self>) {
        let Some(seen) = self.session.as_ref().and_then(|session| session.machine(machine)) else { return };
        let (machine, boot) = (seen.name.clone(), seen.boot.clone());
        let Ok(target) = self.machine_target(seen) else { return };
        if !self.there.homes.ask(&machine, &boot) {
            return;
        }
        run_on(cx, target, args(&["status", "--json"]), move |this: &mut Self, result, cx| {
            let home = result.ok().as_deref().and_then(home_in);
            this.there.homes.answered(&machine, &boot, home);
            cx.notify();
        });
    }
}

#[cfg(test)]
mod tests;
