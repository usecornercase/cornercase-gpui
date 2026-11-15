use gpui::{Context, Pixels, Point, Window};

use super::menus::folder_items;
use crate::app::Root;
use crate::cli;
use crate::context_menu::{Entry, danger, item, resting};
use crate::dialogs::{self, Question};
use crate::model::bots::Bot;
use crate::panels::bot_chat::{answering, ask_new_conversation};
use crate::panels::files::open::Found;
use crate::store::Store;

const NEW_CONVERSATION_ENTRY: &str = "New conversation…";
const RENAME_ENTRY: &str = "Rename bot";
const EDIT_ENTRY: &str = "Edit bot…";
const REMOVE_ENTRY: &str = "Remove bot…";
const REMOVE: &str = "Remove";

pub(super) fn remove_words(name: &str) -> (String, &'static str, &'static str) {
    (
        format!("Remove the bot {name}?"),
        "Its chat, its memory and its folder are deleted. Agents it started keep running.",
        REMOVE,
    )
}

fn ask_remove(id: &str, name: &str, window: &mut Window, cx: &mut gpui::App) {
    let (title, message, button) = remove_words(name);
    let bot = id.to_string();
    let question = Question::new(title, message, button, Vec::new()).on_yes({
        let bot = bot.clone();
        move |cx| Store::get(cx).update(cx, |s, cx| s.remove_bot(&bot, cx))
    });
    dialogs::confirm_naming(&bot, question, window, cx);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuFrom {
    Row,
    Header,
}

fn conversation_entries(bot: &Bot) -> Vec<Entry> {
    if bot.thinking() {
        return vec![resting(answering(&bot.name)), resting(NEW_CONVERSATION_ENTRY)];
    }
    let (id, name) = (bot.id.clone(), bot.name.clone());
    vec![item(NEW_CONVERSATION_ENTRY, move |_, window, cx| ask_new_conversation(&id, &name, window, cx))]
}

fn own_entries(bot: &Bot, from: MenuFrom) -> Vec<Entry> {
    let (renaming, editing) = (bot.id.clone(), bot.id.clone());
    let rename = item(RENAME_ENTRY, move |this, window, cx| this.start_bot_rename(&renaming, window, cx));
    let edit = item(EDIT_ENTRY, move |_, window, cx| dialogs::edit_bot(&editing, window, cx));
    match from {
        MenuFrom::Row => vec![rename, edit],
        MenuFrom::Header => vec![edit],
    }
}

fn removal_entry(bot: &Bot) -> Entry {
    if bot.thinking() {
        return resting(REMOVE_ENTRY);
    }
    let (id, name) = (bot.id.clone(), bot.name.clone());
    danger(REMOVE_ENTRY, move |_, window, cx| ask_remove(&id, &name, window, cx))
}

pub(super) fn bot_menu(bot: &Bot, from: MenuFrom, (openers, here): (&[Found], bool)) -> Vec<Entry> {
    let mut entries = conversation_entries(bot);
    entries.push(Entry::Separator);
    entries.extend(own_entries(bot, from));
    if !bot.folder.is_empty() {
        entries.push(Entry::Separator);
        entries.extend(folder_items(&bot.folder, openers, here));
    }
    entries.extend([Entry::Separator, removal_entry(bot)]);
    entries
}

impl Root {
    pub(super) fn open_bot_menu(&mut self, id: &str, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bot) = self.store.read(cx).bots.find(id).cloned() else { return };
        self.show_bot_menu(&bot, (MenuFrom::Row, at), window, cx);
    }

    pub(crate) fn take_bot_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let asked = self.store.update(cx, |s, _| s.asks.bot_menu.take(&s.bots.list));
        if let Some((bot, at)) = asked {
            self.show_bot_menu(&bot, (MenuFrom::Header, at), window, cx);
        }
    }

    fn show_bot_menu(
        &mut self,
        bot: &Bot,
        (from, at): (MenuFrom, Point<Pixels>),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let openers = self.store.read(cx).openers.clone();
        self.open_menu(at, bot_menu(bot, from, (&openers, cli::host().is_none())), window, cx);
    }
}

#[cfg(test)]
mod tests;
