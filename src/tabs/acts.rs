use gpui::{App, Context, Window};

use crate::app::Root;
use crate::context_menu::{Entry, act, danger, item};
use crate::dialogs::{self, CloseWords};
use crate::keymap;
use crate::model::{MachinePlace, Tab, ThereId, ThereWorkspace};
use crate::sidebar::{Target, cmd, mark_label};
use crate::terminal::cycled;

fn agent_entries(tab: &Tab, switch: Option<Entry>) -> Vec<Entry> {
    let Some(agent) = tab.agent_pane() else { return Vec::new() };
    let t = tab.id.to_string();
    let mark = if agent.pending() {
        act(mark_label(true), cmd(&["mark", "--tab", &t, "--reviewed"]))
    } else {
        act(mark_label(false), cmd(&["mark", "--tab", &t]))
    };
    let pane = agent.id;
    let mut entries = vec![Entry::Separator, mark];
    entries.extend(switch);
    entries.extend(
        dialogs::context_choices(tab)
            .into_iter()
            .map(|(pane, label)| item(label, move |_, window, cx| dialogs::view_context(pane, window, cx))),
    );
    entries.push(danger("Close agent…", move |_, window, cx| crate::sidebar::close_agent(pane, window, cx)));
    entries
}

impl Root {
    pub(super) fn click_tab(&mut self, id: u64, clicks: usize, window: &mut Window, cx: &mut Context<Self>) {
        if clicks == 2 {
            self.start_rename(Target::Tab(id), window, cx);
        } else {
            self.focus_item("tab", id);
        }
    }

    pub(super) fn pick_on_machine(&mut self, place: MachinePlace, cx: &mut Context<Self>) {
        self.store.update(cx, |s, cx| s.pick_machine_place(place, cx));
    }

    fn machine_tab_place(
        &self,
        tab: impl FnOnce(&ThereWorkspace, Option<ThereId>) -> Option<ThereId>,
    ) -> Option<MachinePlace> {
        let (seen, ws) = self.machine_workspace()?;
        let shown = self.side.machine_place.as_ref().and_then(|place| place.tab);
        Some(MachinePlace::at(seen, ws, Some(tab(ws, shown)?)))
    }

    pub(crate) fn pick_machine_tab_number(&mut self, key: &str, cx: &mut Context<Self>) {
        let number =
            |ws: &ThereWorkspace, _| keymap::tab_index(key, ws.tabs.len()).and_then(|i| ws.tabs.get(i)).map(|t| t.id);
        if let Some(place) = self.machine_tab_place(number) {
            self.pick_on_machine(place, cx);
        }
    }

    pub(crate) fn cycle_machine_tab(&mut self, by: i32, cx: &mut Context<Self>) {
        let next = |ws: &ThereWorkspace, shown| cycled(&ws.tabs.iter().map(|t| t.id).collect::<Vec<_>>(), shown, by);
        if let Some(place) = self.machine_tab_place(next) {
            self.pick_on_machine(place, cx);
        }
    }

    pub(crate) fn tab_menu(&self, id: u64, cx: &App) -> Vec<Entry> {
        let Some(tab) = self.session.as_ref().and_then(|s| s.tab(id)).cloned() else { return Vec::new() };
        let mut entries = vec![
            item("Rename tab", move |this, window, cx| this.start_rename(Target::Tab(id), window, cx)),
            act("New shell tab", cmd(&["new-tab", "--focus"])),
        ];
        let switch = tab.agent_pane().and_then(|agent| self.switch_account_item(agent.id, cx));
        entries.extend(agent_entries(&tab, switch));
        entries.push(Entry::Separator);
        entries.push(danger("Close tab", move |this, window, cx| this.close_tab(&tab, window, cx)));
        entries
    }

    pub(crate) fn close_tab(&mut self, tab: &Tab, window: &mut Window, cx: &mut Context<Self>) {
        let command = cmd(&["close", "--tab", &tab.id.to_string()]);
        let Some(agent) = tab.agent_pane() else {
            self.own.queue.cmd(command);
            return;
        };
        let words = CloseWords::tab(tab, Some(agent), None);
        let question = words.question(command).busy(words.busy(format!("close-tab:{}", tab.id)));
        dialogs::confirm(question, window, cx);
    }
}
