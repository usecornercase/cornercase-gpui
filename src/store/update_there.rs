use gpui::Context;

use super::{Busy, Store, ToastKind};
use crate::cli::{self, Said, Target};
use crate::desktop;
use crate::model::machines::Machine;
use crate::platform::Os;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct UpdateThere {
    pub machine: String,
    pub destination: String,
    pub args: Vec<String>,
}

impl UpdateThere {
    pub(crate) fn label(&self) -> String {
        format!("Update cornercase on {}…", self.machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UpdateOffer {
    Hidden,
    Ask(UpdateThere),
    Running(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AfterUpdate {
    Connects,
    Reads,
}

pub(crate) fn update_key(destination: &str) -> String {
    format!("update-there:{destination}")
}

fn updating(name: &str) -> String {
    format!("Updating cornercase on {name}…")
}

pub(crate) fn update_job(destination: &str, name: &str) -> Busy {
    Busy::new(update_key(destination), updating(name))
}

pub(crate) fn update_question(name: &str, after: AfterUpdate, os: Os) -> (String, String, &'static str) {
    let then = match after {
        AfterUpdate::Connects => "The window then connects again.".to_string(),
        AfterUpdate::Reads => format!("The window then reads {name} again."),
    };
    let message = format!(
        "This runs `cornercase update` there from this {}, which installs the latest release and restarts nothing: \
         its server keeps running the one it has until it restarts there. {then}",
        desktop::machine(os)
    );
    (format!("Update cornercase on {name}?"), message, "Update")
}

pub(crate) fn run_update(args: &[String]) -> anyhow::Result<cli::Answer> {
    cli::answer_on(&Target::Local, args)
}

pub(crate) fn offer_of(update: Option<&UpdateThere>, listed: Option<&Machine>, running: bool) -> UpdateOffer {
    let Some(update) = update else { return UpdateOffer::Hidden };
    if running {
        return UpdateOffer::Running(updating(&update.machine));
    }
    match listed {
        Some(machine) if machine.can_update() || machine.updating => UpdateOffer::Hidden,
        _ => UpdateOffer::Ask(update.clone()),
    }
}

pub(crate) fn updated_words(machine: &str, said: &Said) -> (String, ToastKind) {
    match said {
        Said::Done(text) if text.is_empty() => (format!("{machine}: cornercase update ended"), ToastKind::Info),
        Said::Done(text) => (format!("{machine}: {text}"), ToastKind::Info),
        Said::Failed(text) => (format!("Could not update cornercase on {machine}: {text}"), ToastKind::Error),
    }
}

impl Store {
    pub(crate) fn update_offer(&self, note: Option<&super::OlderNote>) -> UpdateOffer {
        let update = note.and_then(|note| note.update.as_deref());
        let listed = update.and_then(|update| self.machines.iter().find(|machine| machine.name == update.machine));
        let running = update.is_some_and(|update| self.busy(&update_key(&update.destination)));
        offer_of(update, listed, running)
    }

    pub(crate) fn update_on_machine(&mut self, update: UpdateThere, cx: &mut Context<Self>) {
        if self.busy(&update_key(&update.destination)) {
            return;
        }
        let (job, args) = (update_job(&update.destination, &update.machine), update.args.clone());
        self.run_then(
            job,
            move || run_update(&args),
            move |this, result, cx| this.updated_on_machine(&update, result, cx),
            cx,
        );
    }

    fn updated_on_machine(
        &mut self,
        update: &UpdateThere,
        result: anyhow::Result<cli::Answer>,
        cx: &mut Context<Self>,
    ) {
        let said = cli::said_of(result);
        let (text, kind) = updated_words(&update.machine, &said);
        self.toast(text, kind, cx);
        if matches!(said, Said::Done(_)) {
            self.probe_after_update(update, cx);
        }
    }
}

#[cfg(test)]
mod tests;
