use gpui::Context;

use super::cycled;
use crate::app::Root;
use crate::app::machine::MachineTab;
use crate::app::machine::acts::MARKING;
use crate::cli;
use crate::keymap::Command;
use crate::model::{PaneInfo, Session};
use crate::sidebar::cmd;
use crate::store::{self, Store, ToastKind};

const NO_AGENT_WAITS: &str = "No agent needs you";
const NO_AGENT_HERE: &str = "No agent in this pane to mark";
const MARKED: &str = "Marked as pending review";
const REVIEWED: &str = "Marked as reviewed";

pub(super) fn project_after(session: &Session, by: i32) -> Option<u64> {
    let shown = session.shown().project;
    cycled(&session.project_order(), shown, by).filter(|next| Some(*next) != shown)
}

pub(super) fn workspace_after(session: &Session, by: i32, removing: impl Fn(u64) -> bool) -> Option<u64> {
    let shown = session.shown();
    let (project, _) = session.workspace(shown.workspace?)?;
    let open: Vec<u64> = project.workspaces.iter().map(|w| w.id).filter(|id| !removing(*id)).collect();
    cycled(&open, shown.workspace, by).filter(|next| Some(*next) != shown.workspace)
}

pub(super) fn mark_pane_args(pane: u64, pending: bool) -> Vec<String> {
    let id = pane.to_string();
    if pending { cmd(&["mark", "--pane", &id, "--reviewed"]) } else { cmd(&["mark", "--pane", &id]) }
}

pub(super) fn mark_args(pane: &PaneInfo) -> Option<(Vec<String>, &'static str)> {
    pane.agent.as_ref()?;
    let pending = pane.pending();
    Some((mark_pane_args(pane.id, pending), if pending { REVIEWED } else { MARKED }))
}

pub(super) fn mark_answer(result: &anyhow::Result<String>, said: &str) -> (String, ToastKind) {
    match result {
        Ok(_) => (said.to_string(), ToastKind::Info),
        Err(e) => (cli::reason(e), ToastKind::Error),
    }
}

impl Root {
    pub(super) fn step_shortcut(&mut self, command: Command, cx: &mut Context<Self>) {
        match command {
            Command::NextWorkspace => self.step_workspace(1, cx),
            Command::PreviousWorkspace => self.step_workspace(-1, cx),
            Command::NextProject => self.step_project(1),
            Command::PreviousProject => self.step_project(-1),
            Command::AgentNeedingYou => self.go_to_agent_needing_you(cx),
            Command::Mark => self.mark_active_pane(cx),
            _ => {}
        }
    }

    fn step_workspace(&mut self, by: i32, cx: &Context<Self>) {
        let store = self.store.read(cx);
        let next = self.session.as_ref().and_then(|s| workspace_after(s, by, |id| store.removing(id)));
        if let Some(workspace) = next {
            self.focus_item("workspace", workspace);
        }
    }

    fn step_project(&mut self, by: i32) {
        if let Some(project) = self.session.as_ref().and_then(|s| project_after(s, by)) {
            self.focus_item("project", project);
        }
    }

    fn go_to_agent_needing_you(&mut self, cx: &mut Context<Self>) {
        let here = self.active_pane().filter(|_| self.machine.is_none());
        match self.session.as_ref().and_then(|s| s.agent_needing_you(here)) {
            Some(pane) => self.focus_item("pane", pane),
            None => self.tell(NO_AGENT_WAITS, cx),
        }
    }

    fn mark_active_pane(&mut self, cx: &mut Context<Self>) {
        if self.machine.is_some() {
            self.mark_active_there(cx);
            return;
        }
        let pane = self.active_pane().and_then(|id| self.session.as_ref()?.pane(id));
        let Some((args, said)) = pane.and_then(mark_args) else {
            self.tell(NO_AGENT_HERE, cx);
            return;
        };
        store::queued(cx, args, move |result, cx| {
            let (text, kind) = mark_answer(&result, said);
            Store::get(cx).update(cx, |s, cx| s.toast(text, kind, cx));
        });
    }

    fn mark_active_there(&mut self, cx: &mut Context<Self>) {
        let Some(run) = self.machine.as_ref().map(MachineTab::run) else { return };
        if !self.still_there(&run, cx) {
            return;
        }
        if let Some(why) = self.boot_refusal(MARKING, cx) {
            self.tell(&why, cx);
            return;
        }
        let pane = self.active_pane().and_then(|id| self.shown_session()?.pane(id));
        let Some((args, said)) = pane.and_then(mark_args) else {
            self.tell(NO_AGENT_HERE, cx);
            return;
        };
        self.send_there(&run, args, Some(said), cx);
    }
}

#[cfg(test)]
mod tests;
