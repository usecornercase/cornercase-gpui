use std::collections::HashMap;

use gpui::{
    App, ClickEvent, Context, Div, FocusHandle, ParentElement as _, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use super::{BotForm, revealed};
use crate::cli;
use crate::dialogs::bot_secret::{Opening, open_secret};
use crate::dialogs::{Question, confirm_over};
use crate::model::bots::{
    Listed, REMOVE_SECRET, SAVED_AT_ONCE, Secret, SecretsSection, SecretsSeen, empty_words, listed_of,
    remove_secret_message, rereads, secret_list_args, secrets_read_failed,
};
use crate::panels::queue;
use crate::store::{self, Busy, Store};
use crate::theme::colours;
use crate::ui::{self, Tone, press};

const LABEL: &str = "Secrets";
const READING: &str = "Reading the secrets…";
const READING_AGAIN: &str = "Reading…";
const TRY_AGAIN: &str = "Try again";
const ADD: &str = "Add…";
const REPLACE: &str = "Replace…";
const REMOVE: &str = "Remove";
const REMOVING: &str = "Removing…";
const ADD_STOP: &str = "bot-secret-add";
const RETRY_STOP: &str = "bot-secrets-retry";
const NAME_WIDEST: f32 = 220.;

pub(super) fn saved_label(at: i64, now: i64) -> Option<String> {
    if at <= 0 {
        return None;
    }
    let (then, today) = (queue::local(at)?, queue::local(now)?);
    Some(format!("saved {}", queue::when_label(then, today)))
}

fn replace_stop(name: &str) -> String {
    format!("bot-secret-replace:{name}")
}

fn remove_stop(name: &str) -> String {
    format!("bot-secret-remove:{name}")
}

enum Kept {
    Reading,
    Read(Listed),
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum At {
    Row(String),
    Add,
    Retry,
}

pub(super) struct SecretsRow {
    kept: Kept,
    reading: bool,
    again: bool,
    serial: u64,
    seen: SecretsSeen,
    add: FocusHandle,
    retry: FocusHandle,
    stops: HashMap<String, [FocusHandle; 2]>,
    gone: Vec<FocusHandle>,
    keyed: Option<At>,
    due: Option<At>,
}

impl SecretsRow {
    pub(super) fn new(cx: &mut App) -> Self {
        Self {
            kept: Kept::Reading,
            reading: false,
            again: false,
            serial: 0,
            seen: SecretsSeen::default(),
            add: cx.focus_handle(),
            retry: cx.focus_handle(),
            stops: HashMap::new(),
            gone: Vec::new(),
            keyed: None,
            due: None,
        }
    }

    fn listed_len(&self) -> Option<usize> {
        match &self.kept {
            Kept::Read(listed) => Some(listed.secrets.len()),
            Kept::Reading | Kept::Failed(_) => None,
        }
    }

    fn keep(&mut self, listed: Listed, cx: &mut App) {
        let names = listed.names();
        for (name, pair) in std::mem::take(&mut self.stops) {
            if names.contains(&name) {
                self.stops.insert(name, pair);
            } else {
                self.gone.extend(pair);
            }
        }
        for name in names {
            self.stops.entry(name).or_insert_with(|| [cx.focus_handle(), cx.focus_handle()]);
        }
        self.kept = Kept::Read(listed);
    }

    fn fail(&mut self, words: String) {
        self.gone.extend(self.stops.drain().flat_map(|(_, pair)| pair));
        self.kept = Kept::Failed(words);
    }

    fn fallback(&self) -> Option<&FocusHandle> {
        match self.kept {
            Kept::Read(_) => Some(&self.add),
            Kept::Failed(_) => Some(&self.retry),
            Kept::Reading => None,
        }
    }

    fn lost(&self, window: &Window) -> bool {
        let read = matches!(self.kept, Kept::Read(_));
        let failed = matches!(self.kept, Kept::Failed(_));
        self.gone.iter().any(|handle| handle.is_focused(window))
            || (self.add.is_focused(window) && !read)
            || (self.retry.is_focused(window) && !failed)
    }

    fn at(&self, window: &Window) -> Option<At> {
        if self.add.is_focused(window) {
            return Some(At::Add);
        }
        if self.retry.is_focused(window) {
            return Some(At::Retry);
        }
        let row = self.stops.iter().find(|(_, pair)| pair.iter().any(|handle| handle.is_focused(window)));
        row.map(|(name, _)| At::Row(name.clone()))
    }

    fn shown(&self, at: &At, scroller: &gpui::ScrollHandle, element: Div) -> Div {
        if self.due.as_ref() == Some(at) { revealed(element, scroller) } else { element }
    }
}

impl BotForm {
    fn secrets_bot(&self) -> Option<String> {
        self.editing().map(|editing| editing.id.clone())
    }

    fn read_secrets(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.secrets_bot() else { return };
        let store = Store::get(cx).read(cx);
        if store.secrets_section() != SecretsSection::Shown {
            return;
        }
        if self.secrets.reading {
            self.secrets.again = true;
            return;
        }
        let name = store.bots.find(&id).map_or_else(|| id.clone(), |bot| bot.name.clone());
        self.secrets.serial += 1;
        self.secrets.reading = true;
        let serial = self.secrets.serial;
        let busy = Busy::new(format!("bot-secrets:{id}"), format!("Reading the secrets of {name}…"));
        store::run_busy(cx, busy, secret_list_args(&id), move |this, result, cx| this.secrets_read(serial, result, cx));
        cx.notify();
    }

    fn secrets_read(&mut self, serial: u64, result: anyhow::Result<String>, cx: &mut Context<Self>) {
        if serial != self.secrets.serial {
            return;
        }
        self.secrets.reading = false;
        match result.and_then(|out| listed_of(&out)) {
            Ok(listed) => self.secrets.keep(listed, cx),
            Err(error) => self.secrets.fail(cli::reason(&error)),
        }
        if std::mem::take(&mut self.secrets.again) {
            self.read_secrets(cx);
        }
        cx.notify();
    }

    fn retry_pressed(&mut self, event: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        if press::pressed(event, RETRY_STOP) {
            self.read_secrets(cx);
        }
    }

    pub(super) fn follow_secrets(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.secrets_bot() else { return };
        let Some(now) = Store::get(cx).read(cx).secrets_seen(&id) else { return };
        let seen = std::mem::replace(&mut self.secrets.seen, now);
        let first = matches!(self.secrets.kept, Kept::Reading) && !self.secrets.reading;
        if first || rereads(self.secrets.listed_len(), seen, now) {
            self.read_secrets(cx);
        }
    }

    pub(super) fn secrets_keys(&mut self, window: &mut Window) {
        let row = &mut self.secrets;
        if row.lost(window)
            && let Some(handle) = row.fallback()
        {
            window.focus(&handle.clone());
        }
        row.gone.clear();
        let now = row.at(window);
        row.due = now.clone().filter(|at| row.keyed.as_ref() != Some(at));
        row.keyed = now;
    }

    pub(super) fn secrets_field(&self, cx: &mut Context<Self>) -> Option<Div> {
        let id = self.secrets_bot()?;
        let body = match Store::get(cx).read(cx).secrets_section() {
            SecretsSection::Hidden => return None,
            SecretsSection::Lacks(words) => ui::dim(words),
            SecretsSection::Shown => match &self.secrets.kept {
                Kept::Reading => ui::loading(READING),
                Kept::Failed(words) => self.secrets_failed(words, cx),
                Kept::Read(listed) => self.secrets_list(&id, listed, cx),
            },
        };
        Some(ui::field(LABEL, body))
    }

    fn secrets_failed(&self, words: &str, cx: &mut Context<Self>) -> Div {
        let button = if self.secrets.reading {
            press::stop_at(
                ui::busy_button(RETRY_STOP, READING_AGAIN, Tone::Plain),
                &self.secrets.retry,
                RETRY_STOP,
                true,
            )
        } else {
            press::stop_at(ui::button(RETRY_STOP, TRY_AGAIN), &self.secrets.retry, RETRY_STOP, true)
                .on_click(cx.listener(Self::retry_pressed))
        };
        let line = self.secrets.shown(&At::Retry, &self.scroller, div().flex().child(button));
        div().flex().flex_col().gap(px(6.)).child(ui::error_line(secrets_read_failed(words))).child(line)
    }

    fn secrets_list(&self, id: &str, listed: &Listed, cx: &Context<Self>) -> Div {
        let names = listed.names();
        let bot = Store::get(cx).read(cx).bots.find(id).map_or_else(|| id.to_string(), |bot| bot.name.clone());
        let now = queue::now();
        let rows: Vec<Div> =
            listed.secrets.iter().filter_map(|secret| self.secret_row((id, &bot, now), secret, &names, cx)).collect();
        let empty = rows.is_empty().then(|| ui::dim(empty_words(&bot)));
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .children(rows)
            .children(empty)
            .child(self.add_line(id, names))
            .child(ui::dim(SAVED_AT_ONCE))
    }

    fn add_line(&self, id: &str, names: Vec<String>) -> Div {
        let (bot, back) = (id.to_string(), self.secrets.add.clone());
        let add = press::stop_at(ui::button(ADD_STOP, ADD), &self.secrets.add, ADD_STOP, true).on_click(
            move |event: &ClickEvent, window, cx| {
                if press::pressed(event, ADD_STOP) {
                    let opening = Opening { bot: bot.clone(), name: None, saved: names.clone(), back: back.clone() };
                    open_secret(opening, window, cx);
                }
            },
        );
        self.secrets.shown(&At::Add, &self.scroller, div().flex().child(add))
    }

    fn secret_row(
        &self,
        (id, bot, now): (&str, &str, i64),
        secret: &Secret,
        names: &[String],
        cx: &Context<Self>,
    ) -> Option<Div> {
        let [replace, remove] = self.secrets.stops.get(&secret.name)?;
        let saved = saved_label(secret.saved, now).unwrap_or_default();
        let row = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(
                div()
                    .flex_none()
                    .text_size(px(12.5))
                    .text_color(colours::text())
                    .child(ui::one_line(secret.name.clone()).widest(px(NAME_WIDEST))),
            )
            .child(div().flex_1().min_w_0().text_size(px(11.)).text_color(colours::muted()).child(ui::one_line(saved)))
            .child(replace_button((id, &secret.name), names, replace))
            .child(remove_button((id, bot, &secret.name), remove, cx.entity_id().as_u64(), cx));
        Some(self.secrets.shown(&At::Row(secret.name.clone()), &self.scroller, row))
    }
}

fn replace_button((id, name): (&str, &str), names: &[String], handle: &FocusHandle) -> Stateful<Div> {
    let stop = replace_stop(name);
    let (bot, secret, saved, back) = (id.to_string(), name.to_string(), names.to_vec(), handle.clone());
    let pressed = stop.clone();
    press::stop_at(ui::button(SharedString::from(stop.clone()), REPLACE), handle, stop, true).on_click(
        move |event: &ClickEvent, window, cx| {
            if press::pressed(event, &pressed) {
                let opening =
                    Opening { bot: bot.clone(), name: Some(secret.clone()), saved: saved.clone(), back: back.clone() };
                open_secret(opening, window, cx);
            }
        },
    )
}

fn remove_button((id, bot, name): (&str, &str, &str), handle: &FocusHandle, form: u64, cx: &App) -> Stateful<Div> {
    let stop = remove_stop(name);
    let element = SharedString::from(stop.clone());
    if Store::get(cx).read(cx).removing_secret(id, name) {
        return press::stop_at(ui::busy_button(element, REMOVING, Tone::Plain), handle, stop, true);
    }
    let (bot_id, bot_name, secret, back) = (id.to_string(), bot.to_string(), name.to_string(), handle.clone());
    let pressed = stop.clone();
    press::stop_at(ui::button(element, REMOVE), handle, stop, true).on_click(move |event: &ClickEvent, _, cx| {
        if press::pressed(event, &pressed) {
            let (bot, name) = (bot_id.clone(), secret.clone());
            let question = Question::new(REMOVE_SECRET, remove_secret_message(&secret, &bot_name), REMOVE, Vec::new())
                .on_yes(move |cx| Store::get(cx).update(cx, |s, cx| s.remove_secret((&bot, &name), form, cx)));
            confirm_over(question, back.clone(), cx);
        }
    })
}

#[cfg(test)]
mod tests;
