use super::data::Entry;
use super::form_view::{EDITED_ONCE, EDITED_ROUTINE};
use crate::cli::{self, Feature, Lacks, Releases};
use crate::platform::Os;

pub(super) fn bot_routines_note(name: &str) -> String {
    format!(
        "A routine only runs while cornercase's server runs. Each run is a new conversation of {name}, which answers \
         in its chat."
    )
}

pub(super) fn elsewhere(entries: &[Entry], bots: &[String]) -> Option<String> {
    let known = |entry: &&Entry| entry.place.bot.as_ref().is_some_and(|bot| bots.contains(bot));
    let count = entries.iter().filter(|entry| entry.of_bot() && entry.routine.is_none()).filter(known).count();
    match count {
        0 => None,
        1 => Some("1 bot routine: in its bot's chat, under Routines".into()),
        many => Some(format!("{many} bot routines: in each bot's chat, under Routines")),
    }
}

pub(super) fn add_note(releases: &Releases<'_>, os: Os) -> Option<String> {
    let Some(Lacks::ThisComputer(has)) = releases.lacks(Feature::BotRoutines) else { return None };
    Some(format!(
        "Scheduling for a bot from this window needs {}.",
        cli::newer_here(Feature::BotRoutines.since(), has, os)
    ))
}

pub(super) fn cleared_too(entries: &[Entry]) -> usize {
    entries.iter().filter(|entry| entry.of_bot() && entry.clearable()).count()
}

pub(super) fn header_text(editing: bool, repeats: bool, name: &str) -> (&'static str, String) {
    match (editing, repeats) {
        (true, true) => ("Edit routine", EDITED_ROUTINE.into()),
        (true, false) => ("Edit reminder", EDITED_ONCE.into()),
        (false, true) => {
            ("Schedule a routine", format!("{name} answers in its chat every time, until you pause or remove it."))
        }
        (false, false) => ("Schedule a reminder", format!("{name} answers in its chat at the time you choose.")),
    }
}

pub(super) fn where_note(repeats: bool, name: &str) -> String {
    if repeats {
        return bot_routines_note(name);
    }
    format!("It only runs while cornercase's server runs, as a new conversation of {name}, which answers in its chat.")
}

#[cfg(test)]
mod tests;
