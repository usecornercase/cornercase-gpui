use std::time::Instant;

use gpui::Context;

use super::PortsPanel;
use super::data::{List, Port};
use super::rows::cornercase_note;
use super::source::{Listed, Scope};
use super::view::there_key;
use crate::cli::{self, Target};
use crate::model::ThereId;
use crate::model::machines::hide_destination;
use crate::model::tunnels::{Gate, gate_note};
use crate::panels::on_machine::Missed;
use crate::panels::poll::Polled as _;
use crate::store::{self, MachineRead, Reach, Store};

pub(super) fn machine_notes(gate: &Gate, machine: &str, reach: &Reach) -> Vec<String> {
    match (gate, reach) {
        (_, Reach::Refused(_)) => Vec::new(),
        (Gate::Older(older), _) => vec![gate_note(older, machine)],
        (_, Reach::Older(_)) | (Gate::Waiting, _) => Vec::new(),
        (Gate::Open, _) => vec![cornercase_note(machine)],
    }
}

pub(super) fn missed_of(machine: &str, destination: &str, error: &anyhow::Error) -> Missed {
    if cli::stale_boot(error) {
        return Missed::Restarted;
    }
    Missed::Failed(hide_destination(&error.to_string(), machine, destination))
}

pub(crate) fn sample_there<T: 'static>(
    cx: &mut Context<T>,
    (read, target, all): (&MachineRead, Target, bool),
    then: impl FnOnce(&mut T, Result<Vec<Port<ThereId>>, Missed>, &mut Context<T>) + 'static,
) {
    let (machine, destination) = (read.aim.machine.clone(), read.destination.clone());
    let (asked, releases) = {
        let store = Store::get(cx).read(cx);
        (store.there_facts(&machine), store.read_releases(&machine))
    };
    store::run_on(cx, target, read.aim.ports_args(all, releases), move |this, result, cx| {
        let list = result.and_then(|out| serde_json::from_str::<List<ThereId>>(&out).map_err(anyhow::Error::from));
        if let Err(e) = &list
            && !cli::stale_boot(e)
        {
            Store::get(cx).update(cx, |s, cx| s.there_refused(&machine, (e, asked), cx));
        }
        then(this, list.map(|list| list.ports).map_err(|e| missed_of(&machine, &destination, &e)), cx);
    });
}

impl PortsPanel {
    pub(super) fn measure_there(&mut self, read: &MachineRead, cx: &mut Context<Self>) {
        let Some(target) = read.reach.target().cloned() else {
            self.retry.forget_while_older(&read.reach);
            cx.notify();
            return;
        };
        if !self.retry.due(Instant::now()) || !self.watch.begin() {
            return;
        }
        let scope = self.scope.clone();
        sample_there(cx, (read, target, self.all), move |this, answer, cx| {
            this.heard_there(scope.as_ref(), answer, cx);
        });
    }

    fn heard_there(
        &mut self,
        scope: Option<&Scope>,
        answer: Result<Vec<Port<ThereId>>, Missed>,
        cx: &mut Context<Self>,
    ) {
        self.watch.end();
        if self.scope.as_ref() != scope {
            self.measure(cx);
            return;
        }
        match answer {
            Ok(ports) => {
                self.retry.heard();
                self.listed_there(ports);
            }
            Err(missed) => self.retry.missed(missed, Instant::now()),
        }
        cx.notify();
    }

    fn listed_there(&mut self, ports: Vec<Port<ThereId>>) {
        self.ticked.retain(|key| ports.iter().any(|port| there_key(port) == *key));
        self.listed = Listed::There(ports);
        self.error = None;
    }
}

#[cfg(test)]
mod tests;
