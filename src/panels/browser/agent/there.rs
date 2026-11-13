use gpui::App;

use super::{Agent, Browser, Hand, toast};
use crate::cli::{self, Target};
use crate::model::machines::hide_destination;
use crate::model::{PaneThere, Session};
use crate::store::{Busy, Store, ToastKind, failed_there};

pub(crate) fn hand_commands(there: &PaneThere, hand: &Hand) -> Vec<Vec<String>> {
    let mut commands: Vec<Vec<String>> = hand.files.iter().map(|file| there.paste_file_args(file)).collect();
    match (hand.submit, hand.text.is_empty()) {
        (true, _) => commands.push(there.send_args(hand.text.clone())),
        (false, false) => commands.push(there.paste_args(&hand.text)),
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

pub(crate) fn run_all(
    commands: &[Vec<String>],
    mut run: impl FnMut(&[String]) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    commands.iter().try_for_each(|command| run(command))
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
        let (machine, commands) = (there.machine.clone(), hand_commands(there, hand));
        let destination = target.host().unwrap_or_default().to_string();
        store.update(cx, |s, cx| {
            s.run_then(
                busy,
                move || run_all(&commands, |command| cli::run_on(&target, command, None).map(drop)),
                move |s, result, cx| match result {
                    Ok(()) => s.toast(said, ToastKind::Info, cx),
                    Err(e) => {
                        let why = hide_destination(&failed_there(&machine, &e), &machine, &destination);
                        s.toast(why, ToastKind::Error, cx);
                    }
                },
                cx,
            );
        });
        true
    }
}

#[cfg(test)]
mod tests;
