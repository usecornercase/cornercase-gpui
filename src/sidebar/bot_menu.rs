use gpui::{Context, Pixels, Point, Window};

use crate::app::Root;
use crate::context_menu::{Entry, danger, item, resting};
use crate::dialogs::{self, Question};
use crate::model::bots::Bot;
use crate::panels::bot_chat::{answering, ask_new_conversation};
use crate::store::Store;

const NEW_CONVERSATION_ENTRY: &str = "New conversation…";
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

pub(super) fn bot_menu(bot: &Bot) -> Vec<Entry> {
    if bot.thinking() {
        return vec![
            resting(answering(&bot.name)),
            resting(NEW_CONVERSATION_ENTRY),
            Entry::Separator,
            resting(REMOVE_ENTRY),
        ];
    }
    let (new_id, new_name) = (bot.id.clone(), bot.name.clone());
    let (gone_id, gone_name) = (bot.id.clone(), bot.name.clone());
    vec![
        item(NEW_CONVERSATION_ENTRY, move |_, window, cx| ask_new_conversation(&new_id, &new_name, window, cx)),
        Entry::Separator,
        danger(REMOVE_ENTRY, move |_, window, cx| ask_remove(&gone_id, &gone_name, window, cx)),
    ]
}

impl Root {
    pub(super) fn open_bot_menu(&mut self, id: &str, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bot) = self.store.read(cx).bots.find(id).cloned() else { return };
        self.open_menu(at, bot_menu(&bot), window, cx);
    }
}

#[cfg(test)]
mod tests;
