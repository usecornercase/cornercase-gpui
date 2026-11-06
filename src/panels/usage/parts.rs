use std::iter::once;

use super::data::{Plan, Plans};
use super::machines::MachineRead;
use crate::model::machines::LinkState;

#[derive(Debug, Clone)]
pub(super) struct Row {
    pub plan: Plan,
    pub read: Option<i64>,
    pub on: Vec<String>,
}

impl Row {
    pub(super) fn status(&self, now: i64) -> String {
        self.plan.status(self.read, now)
    }

    pub(super) fn on_line(&self) -> Option<String> {
        (self.on.len() > 1).then(|| format!("on {}", self.on.join(", ")))
    }

    fn joins(&self, name: &str, other: &Row) -> bool {
        other.plan.identity.is_some()
            && self.plan.identity == other.plan.identity
            && !self.on.iter().any(|on| on == name)
    }

    fn join(&mut self, name: &str, other: Row) {
        self.on.push(name.to_string());
        if other.read.is_some_and(|read| self.read.is_none_or(|kept| read > kept)) {
            let (account, label, active) = (self.plan.account.take(), self.plan.label.take(), self.plan.active);
            self.plan = Plan { account, label, active, ..other.plan };
            self.read = other.read;
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct Part {
    pub name: Option<String>,
    pub state: LinkState,
    pub status: String,
    pub why: Option<String>,
    pub unread: bool,
    pub rows: Vec<Row>,
    pub joined: usize,
}

fn rows(name: &str, plans: &[Plan], machine: Option<&MachineRead>) -> Vec<Row> {
    let row = |plan: &Plan| Row { plan: plan.clone(), read: plan.read(machine), on: vec![name.to_string()] };
    plans.iter().map(row).collect()
}

fn here_part(name: Option<&str>, plans: &[Plan]) -> Part {
    Part {
        name: name.map(str::to_string),
        state: LinkState::Linked,
        status: String::new(),
        why: None,
        unread: false,
        rows: rows(name.unwrap_or_default(), plans, None),
        joined: 0,
    }
}

fn machine_part(machine: &MachineRead, plans: &[Plan], now: i64) -> Part {
    let unread = machine.unread().map(str::to_string);
    Part {
        name: Some(machine.name.clone()),
        state: machine.state,
        status: machine.seen(now).unwrap_or_default(),
        unread: unread.is_some(),
        why: unread.or_else(|| machine.failed(now)),
        rows: rows(&machine.name, plans, Some(machine)),
        joined: 0,
    }
}

fn place(placed: &mut [Part], part: &mut Part) {
    let name = part.name.clone().unwrap_or_default();
    for row in std::mem::take(&mut part.rows) {
        match placed.iter_mut().flat_map(|earlier| &mut earlier.rows).find(|kept| kept.joins(&name, &row)) {
            Some(kept) => {
                kept.join(&name, row);
                part.joined += 1;
            }
            None => part.rows.push(row),
        }
    }
}

fn merged(parts: impl Iterator<Item = Part>) -> Vec<Part> {
    let mut placed = Vec::new();
    for mut part in parts {
        place(&mut placed, &mut part);
        placed.push(part);
    }
    placed
}

pub(super) fn parts(plans: &Plans, here: &str, now: i64) -> Vec<Part> {
    if let Some(machine) = &plans.machine {
        return vec![machine_part(machine, &plans.agents, now)];
    }
    if plans.machines.is_empty() {
        return vec![here_part(None, &plans.agents)];
    }
    let machines = plans.machines.iter().map(|there| machine_part(&there.machine, &there.agents, now));
    merged(once(here_part(Some(here), &plans.agents)).chain(machines))
}

#[cfg(test)]
mod tests;
