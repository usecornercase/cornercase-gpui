use gpui::{Context, Pixels, Point, Window};

use crate::app::Root;
use crate::app::machine::{MachineRun, MachineTab};
use crate::context_menu::{Entry, danger, item, note, resting};
use crate::dialogs::VIEW_CONTEXT;
use crate::render::layout::Hit;
use crate::sidebar::mark_label;

pub(super) const BOOT_WAITS: &str = "needs usecornercase/cornercase-terminal#723";
pub(super) const RESTART_WAITS: &str = "needs usecornercase/cornercase-terminal#729";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct TherePane {
    pub(super) pane: u64,
    pub(super) agent: Option<AgentThere>,
    pub(super) siblings: bool,
    pub(super) path: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct AgentThere {
    pub(super) pending: bool,
    pub(super) on_account: bool,
}

type Act = fn(&mut Root, u64, &mut Window, &mut Context<Root>);

fn guarded(run: &MachineRun, pane: u64, act: Act) -> impl Fn(&mut Root, &mut Window, &mut Context<Root>) + 'static {
    let run = run.clone();
    move |this, window, cx| {
        if this.still_there(&run, cx) {
            act(this, pane, window, cx);
        }
    }
}

fn there_item(label: &'static str, run: &MachineRun, pane: u64, act: Act) -> Entry {
    item(label, guarded(run, pane, act))
}

fn there_danger(label: &'static str, run: &MachineRun, pane: u64, act: Act) -> Entry {
    danger(label, guarded(run, pane, act))
}

fn agent_entries(run: &MachineRun, pane: u64, agent: AgentThere) -> Vec<Entry> {
    let mut entries = vec![Entry::Separator, resting(mark_label(agent.pending)), note(BOOT_WAITS)];
    if agent.on_account {
        entries.extend([resting("Switch account"), note(RESTART_WAITS)]);
    }
    entries.extend([
        there_item(VIEW_CONTEXT, run, pane, Root::view_context_there),
        Entry::Separator,
        there_danger("Close agent…", run, pane, Root::close_agent_there),
    ]);
    entries
}

fn copy_path_item(machine: &str, path: &str) -> Entry {
    let (machine, path) = (machine.to_string(), path.to_string());
    item("Copy path", move |this, _, cx| {
        this.store.update(cx, |s, cx| s.copy_from(path.clone(), &machine, cx));
    })
}

fn move_waits(siblings: bool) -> Vec<Entry> {
    if siblings { vec![resting("Move pane"), note(BOOT_WAITS)] } else { Vec::new() }
}

pub(super) fn there_pane_menu(run: &MachineRun, facts: &TherePane) -> Vec<Entry> {
    let pane = facts.pane;
    let mut entries = vec![
        there_item("Copy", run, pane, |this, pane, _, _| this.copy_selection(pane)),
        there_item("Paste", run, pane, |this, pane, _, cx| this.paste(Some(pane), cx)),
        Entry::Separator,
        there_item("Split right", run, pane, |this, pane, _, _| this.split_pane(pane, false)),
        there_item("Split down", run, pane, |this, pane, _, _| this.split_pane(pane, true)),
    ];
    entries.extend(move_waits(facts.siblings));
    entries.extend(facts.agent.map(|a| agent_entries(run, pane, a)).unwrap_or_default());
    entries.push(Entry::Separator);
    if let Some(path) = facts.path.as_deref() {
        entries.extend([copy_path_item(&run.machine, path), Entry::Separator]);
    }
    entries.push(there_danger("Close pane", run, pane, Root::close_pane_there));
    entries
}

impl Root {
    pub(super) fn open_there_pane_menu(
        &mut self,
        pane: u64,
        at: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let over = matches!(self.hit(at), Some(Hit::Pane(p, _)) if p == pane);
        if !self.takes_input() || !over {
            return;
        }
        let Some(run) = self.machine.as_ref().map(MachineTab::run) else { return };
        let entries = there_pane_menu(&run, &self.there_pane_facts(pane));
        self.open_menu(at, entries, window, cx);
    }

    fn there_pane_facts(&self, pane: u64) -> TherePane {
        let session = self.shown_session();
        let info = session.and_then(|s| s.pane(pane));
        let agent = info
            .filter(|i| i.agent.is_some())
            .map(|i| AgentThere { pending: i.pending(), on_account: i.account.is_some() });
        TherePane {
            pane,
            agent,
            siblings: session.and_then(|s| s.tab_of_pane(pane)).is_some_and(|t| t.panes.len() > 1),
            path: info.and_then(|i| i.path.clone()),
        }
    }
}

#[cfg(test)]
mod tests;
