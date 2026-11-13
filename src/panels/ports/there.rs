use std::time::Instant;

use gpui::Context;

use super::PortsPanel;
use super::data::{List, Port};
use super::source::{Listed, Scope};
use super::view::there_key;
use crate::desktop;
use crate::model::ThereId;
use crate::model::machines::hide_destination;
use crate::model::tunnels::{Gate, gate_note};
use crate::panels::poll::{Polled as _, parse_text};
use crate::store::{self, MachineRead};

pub(super) fn machine_notes(gate: &Gate, machine: &str, refused: bool) -> Vec<String> {
    match gate {
        _ if refused => Vec::new(),
        Gate::Open => vec![format!(
            "Tunnel forwards a port to this {} through cornercase, which lists it; once it is up, Copy and the system \
             browser use it here. The Browser panel does not open a machine's ports yet.",
            desktop::here()
        )],
        Gate::Older(older) => vec![gate_note(older, machine)],
        Gate::Waiting => Vec::new(),
    }
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
        let (scope, read) = (self.scope.clone(), read.clone());
        let args = read.aim.ports_args(self.all);
        store::run_on(cx, target, args, move |this, result, cx| this.heard_there(scope.as_ref(), &read, result, cx));
    }

    fn heard_there(
        &mut self,
        scope: Option<&Scope>,
        read: &MachineRead,
        result: anyhow::Result<String>,
        cx: &mut Context<Self>,
    ) {
        self.watch.end();
        if self.scope.as_ref() != scope {
            self.measure(cx);
            return;
        }
        let answer = parse_text::<List<ThereId>>(result);
        match answer {
            Ok(list) => {
                self.retry.heard();
                self.listed_there(list.ports);
            }
            Err(why) => {
                let said = hide_destination(&why, &read.aim.machine, &read.destination);
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
