use std::time::Instant;

use gpui::Context;

use super::PortsPanel;
use super::data::{List, Port};
use super::rows::cornercase_note;
use super::source::{Listed, Scope};
use super::view::there_key;
use crate::cli::Target;
use crate::model::ThereId;
use crate::model::machines::hide_destination;
use crate::model::tunnels::{Gate, gate_note};
use crate::panels::poll::{Polled as _, parse_text};
use crate::store::{self, MachineRead};

pub(super) fn machine_notes(gate: &Gate, machine: &str, refused: bool) -> Vec<String> {
    match gate {
        _ if refused => Vec::new(),
        Gate::Open => vec![cornercase_note(machine)],
        Gate::Older(older) => vec![gate_note(older, machine)],
        Gate::Waiting => Vec::new(),
    }
}

pub(crate) fn sample_there<T: 'static>(
    cx: &mut Context<T>,
    (read, target, all): (&MachineRead, Target, bool),
    then: impl FnOnce(&mut T, Result<Vec<Port<ThereId>>, String>, &mut Context<T>) + 'static,
) {
    let (machine, destination) = (read.aim.machine.clone(), read.destination.clone());
    store::run_on(cx, target, read.aim.ports_args(all), move |this, result, cx| {
        let list = parse_text::<List<ThereId>>(result).map(|list| list.ports);
        then(this, list.map_err(|why| hide_destination(&why, &machine, &destination)), cx);
    });
}

impl PortsPanel {
    pub(super) fn measure_there(&mut self, read: &MachineRead, cx: &mut Context<Self>) {
        let Some(target) = read.reach.target().cloned() else {
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
        answer: Result<Vec<Port<ThereId>>, String>,
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
            Err(said) => {
                self.retry.failed(said, Instant::now());
            }
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
