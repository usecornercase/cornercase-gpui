use gpui::Context;

use super::cycled;
use crate::app::Root;
use crate::keymap::Command;
use crate::model::{PaneInfo, Session};
use crate::sidebar::cmd;
use crate::store::ToastKind;

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

pub(super) fn mark_args(pane: &PaneInfo) -> Option<(Vec<String>, &'static str)> {
    pane.agent.as_ref()?;
    let id = pane.id.to_string();
    Some(if pane.status.as_deref() == Some("pending") {
        (cmd(&["mark", "--pane", &id, "--reviewed"]), REVIEWED)
    } else {
        (cmd(&["mark", "--pane", &id]), MARKED)
    })
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
        let here = self.active_pane();
        match self.session.as_ref().and_then(|s| s.agent_needing_you(here)) {
            Some(pane) => self.focus_item("pane", pane),
            None => self.tell(NO_AGENT_WAITS, cx),
        }
    }

    fn mark_active_pane(&mut self, cx: &mut Context<Self>) {
        let pane = self.active_pane().and_then(|id| self.session.as_ref()?.pane(id));
        match pane.and_then(mark_args) {
            Some((args, said)) => {
                self.queue.cmd(args);
                self.tell(said, cx);
            }
            None => self.tell(NO_AGENT_HERE, cx),
        }
    }

    fn tell(&self, text: &str, cx: &mut Context<Self>) {
        self.store.update(cx, |s, cx| s.toast(text, ToastKind::Info, cx));
    }
}

#[cfg(test)]
mod tests;
