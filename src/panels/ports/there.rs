use std::time::Instant;

use gpui::Context;

use super::data::{List, Port};
use super::view::{PortView, there_key, view_here, view_there};
use super::{Listed, PortsPanel};
use crate::model::machines::hide_destination;
use crate::model::tunnels::{Gate, gate_note};
use crate::model::{Aim, Session, ThereId};
use crate::panels::poll::{Polled as _, parse_text};
use crate::store::{self, MachineRead};

pub(super) enum Source {
    Here(Option<u64>),
    There(Box<MachineRead>),
}

pub(super) fn source_of(machine: Option<MachineRead>, shown: Option<u64>) -> Source {
    match machine {
        Some(read) => Source::There(Box::new(read)),
        None => Source::Here(shown),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Scope {
    Here { all: bool, workspace: Option<u64> },
    There { all: bool, aim: Aim, destination: String },
}

pub(super) fn scope_of(source: &Source, all: bool) -> Scope {
    match source {
        Source::Here(workspace) => Scope::Here { all, workspace: *workspace },
        Source::There(read) => Scope::There { all, aim: read.aim.clone(), destination: read.destination.clone() },
    }
}

pub(super) fn machine_notes(gate: &Gate, machine: &str) -> Vec<String> {
    match gate {
        Gate::Open => vec![
            "Tunnel forwards a port to this computer through cornercase, which lists it; once it is up, Copy and the \
             system browser use it here. The Browser panel does not open a machine's ports yet."
                .to_string(),
        ],
        Gate::Older(older) => vec![gate_note(older, machine)],
        Gate::Waiting => Vec::new(),
    }
}

pub(super) fn views(listed: &Listed, session: Option<&Session>, aim: Option<&Aim>, all: bool) -> Vec<PortView> {
    match listed {
        Listed::None => Vec::new(),
        Listed::Here(ports) => ports.iter().map(|port| view_here(port, session, all)).collect(),
        Listed::There(ports) => {
            let seen = aim.and_then(|aim| session?.machine(&aim.machine));
            ports.iter().map(|port| view_there(port, seen, all)).collect()
        }
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
