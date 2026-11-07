use std::time::Instant;

use gpui::{AnyView, Context, FocusHandle};

use super::{Busy, Store, ToastKind, run_job, run_job_on, run_with};
use crate::cli::{self, Target};
use crate::model::machines::run_on::clone::{Cloned, Unwritten, clone_args, cloned_note, parse_clone};
use crate::model::machines::run_on::{self, Answer, Awaited, Look, Looking, Offer, Starting, There, parse_answer};
use crate::model::{Project, Spot, ThereId};

pub(crate) struct StartThere {
    pub spot: Spot,
    pub starting: Starting,
    pub command: Vec<String>,
    pub target: Target,
    pub select: bool,
}

pub(crate) struct StartHere {
    pub doing: String,
    pub done: String,
    pub failed: String,
    pub command: Vec<String>,
}

pub(crate) struct CloneThere {
    pub machine: String,
    pub project: u64,
    pub name: String,
    pub url: String,
    pub folder: String,
    pub from: String,
    pub files: Vec<String>,
}

struct CloneJob {
    machine: String,
    boot: String,
    project: u64,
    name: String,
}

fn start_key(dialog: u64) -> String {
    format!("dialog-start:{dialog}")
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
            now: Instant::now(),
        };
        looking.there()
    }

    pub(crate) fn target_of(&self, machine: &str) -> Result<Target, String> {
        let seen = self.session.as_ref().and_then(|session| session.machine(machine));
        self.machine_target(seen.ok_or_else(|| format!("{machine} is not linked"))?)
    }

    pub(crate) fn dialog_starts(&self, dialog: u64) -> bool {
        self.jobs.busy(&start_key(dialog))
    }

    fn shown_dialog(&self) -> Option<u64> {
        self.dialog.as_ref().map(|dialog| dialog.entity_id().as_u64())
    }

    pub(crate) fn shown_dialog_starts(&self) -> bool {
        self.shown_dialog().is_some_and(|dialog| self.dialog_starts(dialog))
    }

    pub(crate) fn start_there(&mut self, start: StartThere, cx: &mut Context<Self>) {
        let dialog = self.shown_dialog();
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
                if start.select {
                    self.awaited = Awaited::answered(&start.spot, &answer, Instant::now());
                    self.follow_awaited(cx);
                }
            }
            Err(why) => self.toast(format!("{machine}: {why}"), ToastKind::Error, cx),
        }
    }

    pub(crate) fn start_here(&mut self, start: StartHere, cx: &mut Context<Self>) {
        let StartHere { doing, done, failed, command } = start;
        let dialog = self.shown_dialog();
        let id = self.start_job(Busy::new(start_key(dialog.unwrap_or_default()), doing), cx);
        let command = self.with_view(command);
        run_job(cx, id, command, move |this: &mut Self, result, cx| match result {
            Ok(_) => {
                this.leave_machine_place(cx);
                this.close_started(dialog, true, cx);
                this.toast(done, ToastKind::Done, cx);
            }
            Err(e) => this.toast(format!("{failed}: {}", cli::reason(&e)), ToastKind::Error, cx),
        });
    }

    fn close_started(&mut self, dialog: Option<u64>, select: bool, cx: &mut Context<Self>) {
        if dialog.is_some() && self.shown_dialog() == dialog {
            self.close_dialog(cx);
            if select {
                self.close_sheet(cx);
            }
        }
    }

    pub(crate) fn follow_awaited(&mut self, cx: &mut Context<Self>) {
        let Some(awaited) = self.awaited.take() else { return };
        let machines = self.session.as_ref().map_or(&[][..], |session| &session.machines);
        match awaited.look(machines, Instant::now()) {
            Look::Listed(place) => self.select_there(place, cx),
            Look::NotYet => self.awaited = Some(awaited),
            Look::Gone => {}
        }
    }

    pub(crate) fn clone_there(&mut self, clone: CloneThere, cx: &mut Context<Self>) {
        let CloneThere { machine, project, name, url, folder, from, files } = clone;
        let target = match self.target_of(&machine) {
            Ok(target) => target,
            Err(why) => return self.not_sent(&why, cx),
        };
        let seen = self.session.as_ref().and_then(|session| session.machine(&machine));
        let boot = seen.map(|seen| seen.boot.clone()).unwrap_or_default();
        let busy = Busy::new(clone_key(&machine, project), format!("Cloning {name} on {machine}…"));
        let id = self.start_job(busy, cx);
        let job = CloneJob { machine, boot, project, name };
        let command = clone_args(&url, &folder, &files, &from);
        let running = (cx.entity(), id);
        let work = move || cli::run_partial_on(&target, &command);
        run_with(cx, Some(running), work, move |this: &mut Self, result, cx| {
            let answer = result.map_err(|e| cli::reason(&e)).and_then(|out| parse_clone(&out));
            this.cloned_there(job, answer, cx);
        });
    }

    fn cloned_there(
        &mut self,
        job: CloneJob,
        answer: Result<(ThereId, Vec<Unwritten>), String>,
        cx: &mut Context<Self>,
    ) {
        let CloneJob { machine, boot, project, name } = job;
        match answer {
            Ok((there, unwritten)) => {
                let kind = if unwritten.is_empty() { ToastKind::Done } else { ToastKind::Error };
                self.toast(cloned_note(&name, &machine, &unwritten), kind, cx);
                self.cloned.push(Cloned { machine, boot, project, there, at: Instant::now() });
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
