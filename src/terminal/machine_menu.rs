use gpui::{Context, Pixels, Point, Window};

use super::pane_accounts::switch_rows;
use super::pane_menu::{move_menu, move_targets};
use super::steps::mark_pane_args;
use crate::app::Root;
use crate::app::machine::acts::SwitchThere;
use crate::app::machine::{MachineRun, MachineTab};
use crate::context_menu::{Entry, danger, item, note, resting, submenu};
use crate::dialogs::VIEW_CONTEXT;
use crate::render::layout::Hit;
use crate::sidebar::mark_label;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct TherePane {
    pub(super) pane: u64,
    pub(super) agent: Option<AgentThere>,
    pub(super) targets: Vec<(u64, String)>,
    pub(super) path: Option<String>,
    pub(super) waits: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct AgentThere {
    pub(super) pending: bool,
    pub(super) switch: SwitchThere,
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

fn guarded_by(
    run: &MachineRun,
    act: impl Fn(&mut Root, &MachineRun, &mut Window, &mut Context<Root>) + 'static,
) -> impl Fn(&mut Root, &mut Window, &mut Context<Root>) + 'static {
    let run = run.clone();
    move |this, window, cx| {
        if this.still_there(&run, cx) {
            act(this, &run, window, cx);
        }
    }
}

fn there_item(label: &'static str, run: &MachineRun, pane: u64, act: Act) -> Entry {
    item(label, guarded(run, pane, act))
}

fn there_danger(label: &'static str, run: &MachineRun, pane: u64, act: Act) -> Entry {
    danger(label, guarded(run, pane, act))
}

fn mark_entries(run: &MachineRun, pane: u64, pending: bool, waits: Option<&str>) -> Vec<Entry> {
    let label = mark_label(pending);
    match waits {
        Some(waits) => vec![resting(label), note(waits.to_string())],
        None => vec![item(
            label,
            guarded_by(run, move |this, run, _, cx| this.send_there(run, mark_pane_args(pane, pending), None, cx)),
        )],
    }
}

fn switch_entries(run: &MachineRun, switch: &SwitchThere) -> Vec<Entry> {
    match switch {
        SwitchThere::Nothing => Vec::new(),
        SwitchThere::Rests(why) => vec![resting("Switch account"), note(why.clone())],
        SwitchThere::Offers(switches) => {
            let run = run.clone();
            let rows = switch_rows(switches.clone(), move |this, switch, cx| {
                if this.still_there(&run, cx) {
                    this.switch_account_there(&run, switch, cx);
                }
            });
            vec![submenu("Switch account", rows)]
        }
    }
}

fn agent_entries(run: &MachineRun, facts: &TherePane, agent: &AgentThere) -> Vec<Entry> {
    let pane = facts.pane;
    let mut entries = vec![Entry::Separator];
    entries.extend(mark_entries(run, pane, agent.pending, facts.waits.as_deref()));
    entries.extend(switch_entries(run, &agent.switch));
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

fn move_entries(run: &MachineRun, facts: &TherePane) -> Vec<Entry> {
    match (&facts.waits, facts.targets.is_empty()) {
        (_, true) => Vec::new(),
        (Some(waits), false) => vec![resting("Move pane"), note(waits.clone())],
        (None, false) => {
            let run = run.clone();
            let act = move |this: &mut Root, pane, aim, cx: &mut Context<Root>| {
                if this.still_there(&run, cx) {
                    this.move_pane(pane, aim, cx);
                }
            };
            move_menu(facts.pane, facts.targets.clone(), &act).into_iter().collect()
        }
    }
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
    entries.extend(move_entries(run, facts));
    entries.extend(facts.agent.iter().flat_map(|agent| agent_entries(run, facts, agent)));
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
        let Some(tab) = self.machine.as_ref() else { return };
        let entries = there_pane_menu(&tab.run(), &self.there_pane_facts(pane, tab));
        let retry = tab.accounts_failed();
        self.open_menu(at, entries, window, cx);
        if retry {
            self.read_accounts_there(cx);
        }
    }

    fn there_pane_facts(&self, pane: u64, tab: &MachineTab) -> TherePane {
        let session = self.shown_session();
        let info = session.and_then(|s| s.pane(pane));
        let agent = info
            .filter(|i| i.agent.is_some())
            .map(|i| AgentThere { pending: i.pending(), switch: tab.switch_there(i) });
        TherePane {
            pane,
            agent,
            targets: move_targets(session, pane),
            path: info.and_then(|i| i.path.clone()),
            waits: tab.boot_note(),
        }
    }
}

#[cfg(test)]
mod tests;
