use gpui::Context;

use super::held::status_words;
use super::{Busy, Heard, Store, ToastKind, listed_gate, listen_to_hold};
use crate::cli::{self, Holding};
use crate::model::machines::hide_destination;
use crate::model::tunnels::{Gate, Toward, held_line, reverse_gate, reverse_note};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReverseState {
    Opening,
    Open { there: u16 },
}

#[derive(Debug, Clone)]
pub(crate) struct ReverseHold {
    pub(crate) destination: String,
    pub(crate) label: String,
    pub(crate) process: u64,
    pub(crate) state: ReverseState,
    pub(crate) job: Option<u64>,
}

pub(crate) fn opened_toast(label: &str, there: u16) -> String {
    format!("Opened this Chrome to {label}: its agents reach it at {}", address_there(there))
}

pub(crate) fn failed_toast(label: &str, why: &str) -> String {
    format!("Could not open this Chrome to {label}: {why}")
}

pub(crate) fn ended_toast(label: &str, why: Option<&str>) -> String {
    match why {
        Some(why) => format!("This Chrome is no longer open to {label}: {why}"),
        None => format!("This Chrome is no longer open to {label}"),
    }
}

pub(crate) fn closed_toast(label: &str) -> String {
    format!("Closed this Chrome to {label}")
}

pub(crate) fn stopped_toast(label: &str) -> String {
    ended_toast(label, Some("it stopped"))
}

pub(crate) fn reverse_reason(said: &str, toward: &Toward, label: &str, destination: &str) -> String {
    let line = cli::reason_of(said);
    let line = match toward {
        Toward::Machine(_) => hide_destination(&line, label, destination),
        Toward::Server => line,
    };
    for lead in ["could not hold a reverse tunnel to port ", "the reverse tunnel of port "] {
        let rest = line.strip_prefix(lead).and_then(|rest| rest.split_once(": "));
        if let Some((_, why)) = rest {
            return why.to_string();
        }
    }
    line
}

pub(crate) fn ending_toast(state: ReverseState, code: Option<i32>, why: &str, label: &str) -> (String, ToastKind) {
    let why = (!why.is_empty()).then_some(why);
    match (state, code) {
        (ReverseState::Opening, _) => {
            let why = why.map_or_else(|| status_words(code), str::to_string);
            (failed_toast(label, &why), ToastKind::Error)
        }
        (ReverseState::Open { .. }, Some(0)) => (ended_toast(label, None), ToastKind::Info),
        (ReverseState::Open { .. }, _) => {
            let why = why.map_or_else(|| status_words(code), str::to_string);
            (ended_toast(label, Some(&why)), ToastKind::Error)
        }
    }
}

pub(crate) fn address_there(there: u16) -> String {
    format!("http://127.0.0.1:{there}")
}

pub(crate) fn copied_words(name: &str) -> String {
    format!("Copied the DevTools address on {name}")
}

impl Store {
    pub(crate) fn reverse_gate_of(&self, toward: &Toward) -> Gate {
        let gate = self.release_gate(toward, reverse_gate);
        match toward {
            Toward::Server => gate,
            Toward::Machine(_) => listed_gate(self.machines_known(), gate),
        }
    }

    pub(crate) fn reverse_refusal(&self, toward: &Toward) -> Option<String> {
        let label = || match toward {
            Toward::Server => cli::host().unwrap_or_default(),
            Toward::Machine(name) => name.clone(),
        };
        match self.reverse_gate_of(toward) {
            Gate::Older(older) => Some(reverse_note(&older, &label())),
            Gate::Waiting => Some(format!("this {}'s cornercase release is not read yet", crate::desktop::here())),
            Gate::Open => self.reached(toward).err(),
        }
    }

    pub(crate) fn reverse_hold(&self, toward: &Toward) -> Option<&ReverseHold> {
        self.tunnels.reverses.get(toward)
    }

    pub(crate) fn open_reverse(&mut self, toward: &Toward, chrome: u16, cx: &mut Context<Self>) {
        if self.tunnels.reverses.contains_key(toward) {
            return;
        }
        if let Some(why) = self.reverse_refusal(toward) {
            self.not_opened(&why, cx);
            return;
        }
        let Ok(reached) = self.reached(toward) else { return };
        let (process, rx) = match Self::spawn_hold(&reached, Holding::Reverse { local: chrome }) {
            Ok(started) => started,
            Err(why) => {
                self.toast(failed_toast(&reached.label, &why), ToastKind::Error, cx);
                return;
            }
        };
        let busy =
            Busy::new(format!("tunnel-reverse:{}", toward.key()), format!("Opening this Chrome to {}…", reached.label));
        let job = Some(self.start_job(busy, cx));
        let state = ReverseState::Opening;
        let hold = ReverseHold { destination: reached.destination, label: reached.label, process, state, job };
        self.tunnels.reverses.insert(toward.clone(), hold);
        listen_to_hold(process, rx, Self::reverse_heard, cx);
        cx.notify();
    }

    pub(crate) fn close_reverse(&mut self, toward: &Toward, cx: &mut Context<Self>) {
        let Some(label) = self.tunnels.reverses.get(toward).map(|hold| hold.label.clone()) else { return };
        self.end_reverse(toward, Some((closed_toast(&label), ToastKind::Info)), cx);
    }

    pub(crate) fn end_reverse(&mut self, toward: &Toward, said: Option<(String, ToastKind)>, cx: &mut Context<Self>) {
        let Some(hold) = self.tunnels.reverses.remove(toward) else { return };
        cli::end_hold(hold.process);
        if let Some(job) = hold.job {
            self.end_job(job, cx);
        }
        if let Some((text, kind)) = said {
            self.toast(text, kind, cx);
        }
        cx.notify();
    }

    pub(crate) fn chrome_stopped(&mut self, cx: &mut Context<Self>) {
        let ending: Vec<(Toward, String)> =
            self.tunnels.reverses.iter().map(|(toward, hold)| (toward.clone(), hold.label.clone())).collect();
        for (toward, label) in ending {
            self.end_reverse(&toward, Some((stopped_toast(&label), ToastKind::Info)), cx);
        }
    }

    fn reverse_heard(&mut self, process: u64, heard: Heard, cx: &mut Context<Self>) {
        let Some(toward) =
            self.tunnels.reverses.iter().find(|(_, hold)| hold.process == process).map(|(t, _)| t.clone())
        else {
            return;
        };
        match heard {
            Heard::Line(line) => {
                if let Some(held) = held_line(&line).filter(|held| held.reverse) {
                    self.reverse_up(&toward, held.port, cx);
                }
            }
            Heard::Ended(ended) => self.reverse_ended(&toward, &ended, cx),
        }
    }

    fn reverse_up(&mut self, toward: &Toward, there: u16, cx: &mut Context<Self>) {
        let Some(hold) = self.tunnels.reverses.get_mut(toward) else { return };
        hold.state = ReverseState::Open { there };
        let (label, job) = (hold.label.clone(), hold.job.take());
        if let Some(job) = job {
            self.end_job(job, cx);
        }
        self.toast(opened_toast(&label, there), ToastKind::Info, cx);
        cx.notify();
    }

    fn reverse_ended(&mut self, toward: &Toward, ended: &cli::Ended, cx: &mut Context<Self>) {
        let Some(hold) = self.tunnels.reverses.remove(toward) else { return };
        if let Some(job) = hold.job {
            self.end_job(job, cx);
        }
        let why = reverse_reason(&ended.said, toward, &hold.label, &hold.destination);
        let (text, kind) = ending_toast(hold.state, ended.code, &why, &hold.label);
        self.toast(text, kind, cx);
        cx.notify();
    }

    pub(super) fn follow_own_reverse(&mut self, ends: bool, cx: &mut Context<Self>) {
        if ends {
            self.end_reverse(&Toward::Server, None, cx);
        }
    }

    pub(super) fn follow_machine_reverse(&mut self, name: &str, cx: &mut Context<Self>) {
        let toward = Toward::Machine(name.to_string());
        let Some(hold) = self.tunnels.reverses.get(&toward) else { return };
        let link = self.machine_link(name).gated(self.reverse_gate_of(&toward));
        if !link.stays_for(&hold.destination) {
            self.end_reverse(&toward, None, cx);
        }
    }
}

#[cfg(test)]
mod tests;
