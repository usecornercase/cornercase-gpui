use gpui::{Context, Pixels, Point, Window};

use super::menus::folder_items;
use crate::app::Root;
use crate::cli;
use crate::context_menu::{Entry, danger, item, resting};
use crate::dialogs::{self, Question};
use crate::model::bots::{Bot, BotKey};
use crate::panels::bot_chat::{answering, ask_new_conversation};
use crate::panels::files::open::Found;
use crate::store::Store;

const NEW_CONVERSATION_ENTRY: &str = "New conversation…";
const RENAME_ENTRY: &str = "Rename bot";
const EDIT_ENTRY: &str = "Edit bot…";
const ROUTINES_ENTRY: &str = "Routines…";
const REMOVE_ENTRY: &str = "Remove bot…";
const REMOVE: &str = "Remove";
const NAMED: usize = 3;

fn scheduled_words(scheduled: &[String]) -> String {
    let named = scheduled.iter().take(NAMED).cloned().collect::<Vec<_>>().join(", ");
    match scheduled.len().saturating_sub(NAMED) {
        0 => named,
        more => format!("{named} and {more} more"),
    }
}

pub(super) fn remove_words(name: &str, scheduled: &[String]) -> (String, String, &'static str) {
    let message = if scheduled.is_empty() {
        "Its chat, its memory and its folder are deleted. Agents it started keep running.".to_string()
    } else {
        format!(
            "Its chat, its memory, its folder and what it scheduled ({}) are deleted. Agents it started keep running.",
            scheduled_words(scheduled)
        )
    };
    (format!("Remove the bot {name}?"), message, REMOVE)
}

fn ask_remove(id: &str, name: &str, window: &mut Window, cx: &mut gpui::App) {
    let scheduled = Store::get(cx).read(cx).bots.runs.scheduled(id).to_vec();
    let (title, message, button) = remove_words(name, &scheduled);
    let bot = id.to_string();
    let question = Question::new(title, message, button, Vec::new()).on_yes({
        let bot = bot.clone();
        move |cx| Store::get(cx).update(cx, |s, cx| s.remove_bot(&bot, cx))
    });
    dialogs::confirm_naming(&BotKey::here(&bot), question, window, cx);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuFrom {
    Row,
    Header,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Offers<'a> {
    pub openers: &'a [Found],
    pub here: bool,
    pub routines: bool,
    pub run_answers: bool,
}

fn conversation_entries(bot: &Bot, run_answers: bool) -> Vec<Entry> {
    if bot.thinking() {
        return vec![resting(answering(&bot.name)), resting(NEW_CONVERSATION_ENTRY)];
    }
    let (key, name) = (bot.key(), bot.name.clone());
    let new = item(NEW_CONVERSATION_ENTRY, move |_, window, cx| ask_new_conversation(&key, &name, window, cx));
    if run_answers { vec![resting(answering(&bot.name)), new] } else { vec![new] }
}

fn own_entries(bot: &Bot, from: MenuFrom, routines: bool) -> Vec<Entry> {
    let (renaming, editing, listing) = (bot.id.clone(), bot.id.clone(), bot.id.clone());
    let rename = item(RENAME_ENTRY, move |this, window, cx| this.start_bot_rename(&renaming, window, cx));
    let edit = item(EDIT_ENTRY, move |_, window, cx| dialogs::edit_bot(&editing, window, cx));
    let routines = routines
        .then(|| item(ROUTINES_ENTRY, move |this, _, cx| this.store.update(cx, |s, cx| s.open_routines(&listing, cx))));
    let mut entries = match from {
        MenuFrom::Row => vec![rename, edit],
        MenuFrom::Header => vec![edit],
    };
    entries.extend(routines);
    entries
}

fn removal_entry(bot: &Bot, run_answers: bool) -> Entry {
    if bot.thinking() || run_answers {
        return resting(REMOVE_ENTRY);
    }
    let (id, name) = (bot.id.clone(), bot.name.clone());
    danger(REMOVE_ENTRY, move |_, window, cx| ask_remove(&id, &name, window, cx))
}

pub(super) fn bot_menu(bot: &Bot, from: MenuFrom, offers: Offers<'_>) -> Vec<Entry> {
    let mut entries = conversation_entries(bot, offers.run_answers);
    entries.push(Entry::Separator);
    entries.extend(own_entries(bot, from, offers.routines));
    if !bot.folder.is_empty() {
        entries.push(Entry::Separator);
        entries.extend(folder_items(&bot.folder, offers.openers, offers.here));
    }
    entries.extend([Entry::Separator, removal_entry(bot, offers.run_answers)]);
    entries
}

impl Root {
    pub(super) fn open_bot_menu(&mut self, id: &str, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bot) = self.store.read(cx).bots.find(id).cloned() else { return };
        self.show_bot_menu(&bot, (MenuFrom::Row, at), window, cx);
    }

    pub(crate) fn take_bot_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let asked = self.store.update(cx, |s, _| s.asks.bot_menu.take(&s.bots.list));
        if let Some((bot, (at, keys))) = asked {
            self.show_bot_menu(&bot, (MenuFrom::Header, at), window, cx);
            if keys {
                self.light_first(cx);
            }
        }
    }

    fn show_bot_menu(
        &mut self,
        bot: &Bot,
        (from, at): (MenuFrom, Point<Pixels>),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.read(cx);
        let (openers, routines) = (store.openers.clone(), store.routines_offered());
        let run_answers = store.bots.runs.answering(&bot.id);
        let offers = Offers { openers: &openers, here: cli::host().is_none(), routines, run_answers };
        self.open_menu(at, bot_menu(bot, from, offers), window, cx);
    }
}

#[cfg(test)]
mod tests;
