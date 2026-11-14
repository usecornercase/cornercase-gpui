use gpui::App;

use super::{Agent, Browser, Hand, toast};
use crate::cli::{self, Target, Version};
use crate::model::machines::hide_destination;
use crate::model::{PaneThere, Session};
use crate::store::{Busy, Store, ToastKind, failed_there, part_sent_note};

pub(crate) fn hand_commands(there: &PaneThere, hand: &Hand, release: Option<Version>) -> Vec<Vec<String>> {
    let mut commands: Vec<Vec<String>> = hand.files.iter().map(|file| there.paste_file_args(file, release)).collect();
    match (hand.submit, hand.text.is_empty()) {
        (true, _) => commands.push(there.send_args(hand.text.clone(), release)),
        (false, false) => commands.push(there.paste_args(&hand.text, release)),
        (false, true) => {}
    }
    commands
}

enum Refusal {
    NotSent(String),
    Said(String),
}

fn cleared(store: &Store, there: &PaneThere) -> Result<Target, Refusal> {
    let session: Option<&Session> = store.session.as_ref();
    let machines = session.map_or(&[][..], |session| &session.machines);
    let seen = there.sighted(machines).map_err(Refusal::NotSent)?;
    if let Some(why) = super::unready_pane(seen.pane(there.pane)) {
        return Err(Refusal::Said(why));
    }
    store.machine_target(seen).map_err(Refusal::NotSent)
}

pub(crate) struct Stopped {
    pub sent: usize,
    pub error: anyhow::Error,
}

pub(crate) fn run_all(
    commands: &[Vec<String>],
    mut run: impl FnMut(&[String]) -> anyhow::Result<()>,
) -> Result<(), Stopped> {
    for (sent, command) in commands.iter().enumerate() {
        run(command).map_err(|error| Stopped { sent, error })?;
    }
    Ok(())
}

pub(crate) fn hand_refusal(machine: &str, destination: &str, stopped: &Stopped) -> String {
    match (cli::stale_boot(&stopped.error), stopped.sent) {
        (true, 0) | (false, _) => hide_destination(&failed_there(machine, &stopped.error), machine, destination),
        (true, _) => part_sent_note(machine),
    }
}

impl Browser {
    pub(super) fn hand_there(there: &PaneThere, hand: &Hand, said: String, cx: &mut App) -> bool {
        let store = Store::get(cx);
        let target = match cleared(store.read(cx), there) {
            Ok(target) => target,
            Err(Refusal::NotSent(why)) => {
                store.update(cx, |s, cx| s.not_sent(&why, cx));
                return false;
            }
            Err(Refusal::Said(why)) => {
                toast(why, ToastKind::Error, cx);
                return false;
            }
        };
        let name = Self::agent_name(&Agent::There(there.clone()), cx);
        let key = format!("browser-hand:{}", there.key());
        if store.read(cx).busy(&key) {
            let why = format!("still handing to {name} on {}", there.machine);
            store.update(cx, |s, cx| s.not_sent(&why, cx));
            return false;
        }
        let busy = Busy::new(key, format!("Handing to {name} on {}…", there.machine));
        let release = store.read(cx).cli_release();
        let (machine, commands) = (there.machine.clone(), hand_commands(there, hand, release));
        let destination = target.host().unwrap_or_default().to_string();
        store.update(cx, |s, cx| {
            s.run_then(
                busy,
                move || Ok(run_all(&commands, |command| cli::run_on(&target, command, None).map(drop))),
                move |s, result, cx| match result {
                    Ok(Ok(())) => s.toast(said, ToastKind::Info, cx),
                    Ok(Err(stopped)) => s.toast(hand_refusal(&machine, &destination, &stopped), ToastKind::Error, cx),
                    Err(e) => s.toast(failed_there(&machine, &e), ToastKind::Error, cx),
                },
                cx,
            );
        });
        true
    }
}

#[cfg(test)]
mod tests;
