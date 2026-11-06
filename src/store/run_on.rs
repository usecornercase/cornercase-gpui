use std::time::Instant;

use gpui::{AnyView, Context, FocusHandle};

use super::{Busy, Store, ToastKind, run_job_on};
use crate::cli::{self, Target};
use crate::model::machines::run_on::clone::{Cloned, clone_args, parse_clone};
use crate::model::machines::run_on::{self, Answer, Awaited, Look, Looking, Offer, Starting, There, parse_answer};
use crate::model::{Project, Spot, ThereId};

pub(crate) struct StartThere {
    pub spot: Spot,
    pub starting: Starting,
    pub command: Vec<String>,
    pub target: Target,
    pub select: bool,
}

pub(crate) struct CloneThere {
    pub machine: String,
    pub project: u64,
    pub name: String,
    pub url: String,
    pub folder: String,
}

struct CloneJob {
    machine: String,
    boot: String,
    project: u64,
    name: String,
}

fn start_key(dialog: u64) -> String {
    format!("start-there:{dialog}")
}

fn clone_key(machine: &str, project: u64) -> String {
    format!("clone-there:{machine}:{project}")
}

impl Store {
    pub(crate) fn offers(&self) -> Vec<Offer> {
        let seen = self.session.as_ref().map_or(&[][..], |session| &session.machines);
        run_on::offers(&self.machines, seen, cli::host().as_deref(), crate::panels::queue::now())
    }

    pub(crate) fn project_here(&self, id: Option<u64>) -> Option<&Project> {
        self.session.as_ref()?.projects.iter().find(|project| Some(project.id) == id)
    }

    pub(crate) fn there(&self, machine: &str, project: Option<u64>) -> There {
        let looking = Looking {
            machine,
            project: self.project_here(project),
            list: &self.machines,
            seen: self.session.as_ref().map_or(&[][..], |session| &session.machines),
            cloning: project.is_some_and(|project| self.jobs.busy(&clone_key(machine, project))),
            cloned: &self.cloned,
        };
        looking.there()
    }

    pub(crate) fn target_of(&self, machine: &str) -> Result<Target, String> {
        let seen = self.session.as_ref().and_then(|session| session.machine(machine));
        self.machine_target(seen.ok_or_else(|| format!("{machine} is not linked"))?)
    }

    pub(crate) fn starting_there(&self, dialog: u64) -> bool {
        self.jobs.busy(&start_key(dialog))
    }

    fn shown_dialog(&self) -> Option<u64> {
        self.dialog.as_ref().map(|dialog| dialog.entity_id().as_u64())
    }

    pub(crate) fn start_there(&mut self, start: StartThere, cx: &mut Context<Self>) {
        let dialog = self.shown_dialog();
        let early = start.select && start.starting.waits_for_its_agent();
        self.awaited = early.then(|| Awaited::new_tab_in(&start.spot, dialog, Instant::now()));
        let busy = Busy::new(start_key(dialog.unwrap_or_default()), start.starting.doing(&start.spot.machine));
        let id = self.start_job(busy, cx);
        let (target, command) = (start.target.clone(), start.command.clone());
        run_job_on(cx, id, target, command, move |this: &mut Self, result, cx| {
            let answer = result.map_err(|e| cli::reason(&e)).and_then(|out| parse_answer(&out));
            this.started_there(&start, dialog, answer, cx);
        });
    }

    fn started_there(
        &mut self,
        start: &StartThere,
        dialog: Option<u64>,
        answer: Result<Answer, String>,
        cx: &mut Context<Self>,
    ) {
        let machine = &start.spot.machine;
        match answer {
            Ok(answer) => {
                self.close_started(dialog, start.select, cx);
                if let Some(note) = answer.note.as_deref() {
                    self.toast(note, ToastKind::Info, cx);
                }
                self.toast(start.starting.done(machine, &answer), ToastKind::Done, cx);
                self.await_answer(start, &answer, cx);
            }
            Err(why) => {
                self.toast(format!("{machine}: {why}"), ToastKind::Error, cx);
                self.drop_awaited_of(&start.spot);
            }
        }
    }

    fn close_started(&mut self, dialog: Option<u64>, select: bool, cx: &mut Context<Self>) {
        if dialog.is_some() && self.shown_dialog() == dialog {
            self.close_dialog(cx);
            if select {
                self.close_sheet(cx);
            }
        }
    }

    fn await_answer(&mut self, start: &StartThere, answer: &Answer, cx: &mut Context<Self>) {
        let followed_early = start.starting.waits_for_its_agent();
        let still_waits = self.awaited.as_ref().is_some_and(|awaited| awaited.takes_answer_of(&start.spot));
        if start.select && (!followed_early || still_waits) {
            self.awaited = Awaited::answered(&start.spot, answer, Instant::now());
            self.follow_awaited(cx);
        }
    }

    fn drop_awaited_of(&mut self, spot: &Spot) {
        self.awaited.take_if(|awaited| awaited.takes_answer_of(spot));
    }

    pub(crate) fn follow_awaited(&mut self, cx: &mut Context<Self>) {
        let Some(awaited) = self.awaited.take() else { return };
        let machines = self.session.as_ref().map_or(&[][..], |session| &session.machines);
        match awaited.look(machines, Instant::now()) {
            Look::Listed(place) => {
                self.close_started(awaited.dialog, true, cx);
                self.select_there(place, cx);
            }
            Look::NotYet => self.awaited = Some(awaited),
            Look::Gone => {}
        }
    }

    pub(crate) fn clone_there(&mut self, clone: CloneThere, cx: &mut Context<Self>) {
        let CloneThere { machine, project, name, url, folder } = clone;
        let target = match self.target_of(&machine) {
            Ok(target) => target,
            Err(why) => return self.not_sent(&why, cx),
        };
        let seen = self.session.as_ref().and_then(|session| session.machine(&machine));
        let boot = seen.map(|seen| seen.boot.clone()).unwrap_or_default();
        let busy = Busy::new(clone_key(&machine, project), format!("Cloning {name} on {machine}…"));
        let id = self.start_job(busy, cx);
        let job = CloneJob { machine, boot, project, name };
        run_job_on(cx, id, target, clone_args(&url, &folder), move |this: &mut Self, result, cx| {
            let answer = result.map_err(|e| cli::reason(&e)).and_then(|out| parse_clone(&out));
            this.cloned_there(job, answer, cx);
        });
    }

    fn cloned_there(&mut self, job: CloneJob, answer: Result<ThereId, String>, cx: &mut Context<Self>) {
        let CloneJob { machine, boot, project, name } = job;
        match answer {
            Ok(there) => {
                self.toast(format!("Cloned {name} on {machine}"), ToastKind::Done, cx);
                self.cloned.push(Cloned { machine, boot, project, there });
            }
            Err(why) => self.toast(format!("The clone on {machine} failed: {why}"), ToastKind::Error, cx),
        }
    }

    pub(crate) fn show_dialog_over(
        &mut self,
        view: AnyView,
        focus: FocusHandle,
        back: FocusHandle,
        cx: &mut Context<Self>,
    ) {
        self.dialog_behind = self.dialog.take().map(|dialog| (dialog, back));
        self.dialog = Some(view);
        self.refocus = Some(focus);
        cx.notify();
    }

    pub(super) fn dialog_comes_back(&mut self) {
        if let Some((dialog, focus)) = self.dialog_behind.take() {
            self.dialog = Some(dialog);
            self.refocus = Some(focus);
        }
    }
}
