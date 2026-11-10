use gpui::{App, Context};

use super::{MachineRun, MachineTab};
use crate::app::Root;
use crate::cli::Version;
use crate::desktop;
use crate::model::accounts::{AccountsInfo, Switch};
use crate::model::machines::run_on::checks::Asked;
use crate::model::{PaneInfo, with_boot};
use crate::store::{self, Busy, Store, ToastKind, args, failed_there};

pub(crate) const PANE_BOOT_SINCE: Version = Version::new(0, 13, 30);
pub(crate) const MOVING: &str = "Moving a pane";
pub(crate) const MARKING: &str = "Marking an agent";
pub(crate) const SWITCHING: &str = "Switching the account";

pub(crate) fn sends_boot(release: Option<Version>) -> bool {
    PANE_BOOT_SINCE.reached_by(release)
}

pub(crate) fn boot_note(release: Option<Version>) -> Option<String> {
    (!sends_boot(release)).then(|| format!("needs cornercase {PANE_BOOT_SINCE} or newer on this {}", desktop::here()))
}

pub(crate) fn boot_refusal(doing: &str, machine: &str, release: Option<Version>) -> Option<String> {
    let note = boot_note(release)?;
    let has = release.map(|release| format!(" (it has {release})")).unwrap_or_default();
    Some(format!("{doing} on {machine} {note}{has}"))
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum SwitchThere {
    #[default]
    Nothing,
    Rests(String),
    Offers(Vec<Switch>),
}

pub(crate) fn switch_there(
    accounts: &Asked<AccountsInfo>,
    pane: &PaneInfo,
    machine: &str,
    note: Option<&str>,
) -> SwitchThere {
    match accounts {
        Asked::Read(info) => match (info.switches_of(pane, None), note) {
            (switches, _) if switches.is_empty() => SwitchThere::Nothing,
            (_, Some(note)) => SwitchThere::Rests(note.to_string()),
            (switches, None) => SwitchThere::Offers(switches),
        },
        Asked::Unasked | Asked::Asking if pane.account.is_some() => {
            SwitchThere::Rests(format!("Asking {machine} for its accounts…"))
        }
        Asked::Unasked | Asked::Asking => SwitchThere::Nothing,
        Asked::Failed(why) => SwitchThere::Rests(format!("Could not read {machine}'s accounts: {why}")),
    }
}

pub(crate) fn switch_command(switch: &Switch, run: &MachineRun) -> Vec<String> {
    with_boot(switch.args(), &run.boot)
}

pub(crate) fn there_answer(
    machine: &str,
    result: &anyhow::Result<String>,
    said: Option<&str>,
) -> Option<(String, ToastKind)> {
    match result {
        Err(e) => Some((failed_there(machine, e), ToastKind::Error)),
        Ok(_) => said.map(|text| (text.to_string(), ToastKind::Info)),
    }
}

pub(crate) fn switch_key(run: &MachineRun, pane: u64) -> String {
    format!("account-restart:{}:{}:{pane}", run.machine, run.boot)
}

fn switched(machine: &str, switch: &Switch, who: &str, result: anyhow::Result<String>) -> (String, ToastKind) {
    match switch.answered(result, who) {
        Ok(said) => (said.text, if said.refused { ToastKind::Error } else { ToastKind::Info }),
        Err(e) => (failed_there(machine, &e), ToastKind::Error),
    }
}

impl MachineTab {
    fn ask_accounts(&mut self) -> (u64, u64) {
        self.accounts = Asked::Asking;
        self.accounts_asks += 1;
        (self.conn.queue_serial(), self.accounts_asks)
    }

    fn take_accounts(&mut self, serial: u64, ask: u64, result: anyhow::Result<String>) -> bool {
        let current = self.conn.queue_serial() == serial && self.accounts_asks == ask;
        if current {
            self.accounts = Asked::answered(result);
        }
        current
    }

    pub(crate) fn accounts_failed(&self) -> bool {
        matches!(self.accounts, Asked::Failed(_))
    }

    pub(crate) fn switch_there(&self, pane: &PaneInfo, release: Option<Version>) -> SwitchThere {
        switch_there(&self.accounts, pane, &self.machine, boot_note(release).as_deref())
    }

    pub(crate) fn send_on_run(
        &self,
        command: Vec<String>,
        reply: impl FnOnce(anyhow::Result<String>) + Send + 'static,
    ) {
        self.conn.queue.cmd_then(with_boot(command, &self.boot), reply);
    }
}

impl Root {
    pub(crate) fn cli_release(&self, cx: &App) -> Option<Version> {
        self.store.read(cx).cli_release()
    }

    pub(crate) fn boot_refusal(&self, doing: &str, cx: &App) -> Option<String> {
        let tab = self.machine.as_ref()?;
        boot_refusal(doing, &tab.machine, self.cli_release(cx))
    }

    pub(crate) fn read_accounts_there(&mut self, cx: &mut Context<Self>) {
        let Some(tab) = self.machine.as_mut() else { return };
        let (serial, ask) = tab.ask_accounts();
        let target = tab.conn.target().clone();
        store::run_on(cx, target, args(&["account", "list", "--json"]), move |this: &mut Root, result, cx| {
            if this.machine.as_mut().is_some_and(|tab| tab.take_accounts(serial, ask, result)) {
                cx.notify();
            }
        });
    }

    pub(super) fn watch_accounts_there(&mut self, opened: bool, cx: &mut Context<Self>) {
        let Some(tab) = self.machine.as_mut() else { return };
        let moved = tab.session.as_ref().is_some_and(|session| tab.config_moves.view_line(session));
        if opened || moved {
            self.read_accounts_there(cx);
        }
    }

    pub(crate) fn send_there(
        &mut self,
        run: &MachineRun,
        command: Vec<String>,
        said: Option<&'static str>,
        cx: &mut Context<Self>,
    ) {
        let machine = run.machine.clone();
        let reply = store::reply(cx, move |result, cx| {
            if let Some((text, kind)) = there_answer(&machine, &result, said) {
                Store::get(cx).update(cx, |s, cx| s.toast(text, kind, cx));
            }
        });
        if let Some(tab) = self.machine.as_ref().filter(|tab| tab.run() == *run) {
            tab.send_on_run(command, reply);
        }
    }

    pub(crate) fn switch_account_there(&mut self, run: &MachineRun, switch: Switch, cx: &mut Context<Self>) {
        let Some(tab) = self.machine.as_ref().filter(|tab| tab.run() == *run) else { return };
        if let Some(why) = self.boot_refusal(SWITCHING, cx) {
            self.tell(&why, cx);
            return;
        }
        let who = format!("{} on {}", switch.agent, run.machine);
        let busy = Busy::new(switch_key(run, switch.pane), switch.doing_as(&who));
        let command = switch_command(&switch, run);
        let machine = run.machine.clone();
        store::run_on_busy(cx, busy, tab.conn.target().clone(), command, move |_: &mut Root, result, cx| {
            let (text, kind) = switched(&machine, &switch, &who, result);
            Store::get(cx).update(cx, |s, cx| s.toast(text, kind, cx));
        });
    }
}

#[cfg(test)]
mod tests;
