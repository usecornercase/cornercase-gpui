use gpui::{Context, Pixels, Point, Window};

use super::Renaming;
use crate::app::Root;
use crate::context_menu::{Entry, item, resting};
use crate::dialogs::{self, Question};
use crate::model::machines::seen::sighted;
use crate::model::{Aim, PaneAim, ThereWorkspace};
use crate::panels::worktrees::{self, Whose, WorktreeRow};
use crate::store::{MachineAct, Store};

const RENAME_WORKSPACE: &str = "Rename workspace";

fn sentence(count: usize, one: &str, many: impl Fn(usize) -> String) -> String {
    match count {
        0 => String::new(),
        1 => one.to_string(),
        n => many(n),
    }
}

fn close_question(ws: &ThereWorkspace, machine: &str) -> (String, String) {
    let tabs = sentence(ws.tabs.len(), "Its tab and the programs running in it stop. ", |n| {
        format!("Its {n} tabs and the programs running in them stop. ")
    });
    let closed = sentence(ws.closed_agents.unwrap_or(0), " Its closed agent is forgotten.", |n| {
        format!(" Its {n} closed agents are forgotten.")
    });
    let title = format!("Close the workspace {} on {machine}?", ws.name);
    (title, format!("{tabs}Its folder stays on disk there.{closed}"))
}

fn there_entries(reach: Result<(), String>, aim: Aim) -> Vec<Entry> {
    match reach {
        Ok(()) => vec![item(RENAME_WORKSPACE, move |this, window, cx| this.start_there_rename(&aim, window, cx))],
        Err(why) => vec![resting(why), resting(RENAME_WORKSPACE)],
    }
}

fn there_agent_entries(reach: Result<(), String>, aim: PaneAim) -> Vec<Entry> {
    let label = aim.label();
    match reach {
        Ok(()) => vec![item(label, move |this, _, cx| {
            this.store.update(cx, |s, cx| s.fold_on_machine(aim.clone(), cx));
        })],
        Err(why) => vec![resting(why), resting(label)],
    }
}

impl Root {
    pub(super) fn open_there_agent_menu(
        &mut self,
        aim: &PaneAim,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let reach = self.store.read(cx).reach_pane(aim).map(|_| ());
        self.open_menu(at, there_agent_entries(reach, aim.clone()), window, cx);
    }

    pub(super) fn open_there_menu(
        &mut self,
        aim: &Aim,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let reach = self.store.read(cx).reach_machine(aim).map(|_| ());
        self.open_menu(at, there_entries(reach, aim.clone()), window, cx);
    }

    pub(super) fn start_there_rename(&mut self, aim: &Aim, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(why) = self.store.read(cx).reach_machine(aim) {
            self.store.update(cx, |s, cx| s.not_sent(&why, cx));
            return;
        }
        let machines = self.session.as_ref().map_or(&[][..], |session| &session.machines);
        let Ok((_, ws)) = sighted(machines, aim) else { return };
        let initial = ws.given_name().to_string();
        self.open_rename(Renaming::There(aim.clone()), initial, window, cx);
    }

    pub(super) fn close_there(&mut self, aim: &Aim, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(why) = self.store.read(cx).reach_machine(aim) {
            self.store.update(cx, |s, cx| s.not_sent(&why, cx));
            return;
        }
        let machines = self.session.as_ref().map_or(&[][..], |session| &session.machines);
        let Ok((seen, ws)) = sighted(machines, aim) else { return };
        if ws.worktree {
            let row = WorktreeRow { whose: Whose::There(aim.clone()), path: ws.path.clone(), name: ws.name.clone() };
            worktrees::remove_worktree(row, window, cx);
            return;
        }
        let (title, message) = close_question(ws, &seen.name);
        let aim = aim.clone();
        let question = Question::new(title, message, "Close workspace", Vec::new()).on_yes(move |cx| {
            Store::get(cx).update(cx, |s, cx| s.act_on_machine(aim.clone(), MachineAct::Close, cx));
        });
        dialogs::confirm(question, window, cx);
    }
}

#[cfg(test)]
mod tests;
