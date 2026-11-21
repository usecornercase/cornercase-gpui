use std::collections::HashMap;

use gpui::{
    AnyElement, App, AppContext as _, ClickEvent, Context, Div, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement, KeyDownEvent, ParentElement as _, Render, ScrollHandle, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use self::started::{started, started_stops};
use super::{body_height, close, edit_bot, fitted, frame, revealed, show};
use crate::cli;
use crate::desktop;
use crate::model::bots::{
    AboutGate, Bot, BotKey, CardRow, Controls, Home, ListRow, StartedRow, Tone as RowTone, about_failed, about_of,
    card_rows, controls, folder_parts, folder_shown, routine_rows,
};
use crate::panels::bot_chat::time_label;
use crate::panels::queue;
use crate::store::{Clip, Store};
use crate::theme::colours;
use crate::ui::{self, press};

const COPY_STOP: &str = "bot-card-copy";
const RETRY_STOP: &str = "bot-card-retry";
const ROUTINES_STOP: &str = "bot-card-routines";
const EDIT_STOP: &str = "bot-card-edit";
const STOPS: [&str; 4] = [COPY_STOP, RETRY_STOP, ROUTINES_STOP, EDIT_STOP];
const LABEL_WIDTH: f32 = 84.;
const FOR: &str = "What it is for";
const NOTHING_YET: &str = "Nothing written yet.";
const READING: &str = "Reading…";
const TRY_AGAIN: &str = "Try again";
const ROUTINES_TITLE: &str = "Routines";
const NO_ROUTINES: &str = "No routines yet";

mod started;

fn card_title(name: &str) -> String {
    format!("The bot {name}")
}

enum About {
    Hidden,
    Lacks(String),
    Reading,
    Read(String),
    Failed(String),
}

pub(crate) struct BotCard {
    bot: BotKey,
    about: About,
    serial: u64,
    focus: FocusHandle,
    stops: HashMap<SharedString, FocusHandle>,
    keyed: Option<SharedString>,
    due: Option<SharedString>,
    drawn: Vec<SharedString>,
    scroller: ScrollHandle,
}

pub(crate) fn bot_card(key: &BotKey, window: &mut Window, cx: &mut App) {
    if Store::get(cx).read(cx).bot(key).is_none() {
        return;
    }
    let view = cx.new(|cx| BotCard::new(key, cx));
    let focus = view.read(cx).focus.clone();
    show(view.into(), &focus, window, cx);
    Store::get(cx).update(cx, |s, _| s.names_bot(key));
}

impl BotCard {
    fn new(key: &BotKey, cx: &mut Context<Self>) -> Self {
        cx.observe(&Store::get(cx), |_, _, cx| cx.notify()).detach();
        let about = match Store::get(cx).read(cx).about_gate(&key.home) {
            AboutGate::Hidden => About::Hidden,
            AboutGate::Lacks(words) => About::Lacks(words),
            AboutGate::Read => About::Reading,
        };
        let mut card = Self {
            bot: key.clone(),
            about,
            serial: 0,
            focus: cx.focus_handle(),
            stops: STOPS.iter().map(|name| (SharedString::from(*name), cx.focus_handle())).collect(),
            keyed: None,
            due: None,
            drawn: Vec::new(),
            scroller: ScrollHandle::new(),
        };
        if matches!(card.about, About::Reading) {
            card.read_about(cx);
        }
        card
    }

    fn read_about(&mut self, cx: &mut Context<Self>) {
        self.serial += 1;
        self.about = About::Reading;
        let (serial, card, key) = (self.serial, cx.weak_entity(), self.bot.clone());
        let then = move |_: &mut Store, result, cx: &mut Context<Store>| {
            let _ = card.update(cx, |card, cx| card.about_read(serial, result, cx));
        };
        Store::get(cx).update(cx, |s, cx| s.read_bot_about(&key, then, cx));
        cx.notify();
    }

    fn about_read(&mut self, serial: u64, result: anyhow::Result<String>, cx: &mut Context<Self>) {
        if serial != self.serial {
            return;
        }
        self.about = match result.and_then(|out| about_of(&out)) {
            Ok(text) => About::Read(text),
            Err(error) => About::Failed(about_failed(&cli::reason(&error))),
        };
        cx.notify();
    }

    fn reveal(&mut self, window: &Window) {
        let now = self.stops.iter().find(|(_, handle)| handle.is_focused(window)).map(|(name, _)| name.clone());
        self.due = now.clone().filter(|name| self.keyed.as_ref() != Some(name));
        self.keyed = now;
    }

    fn drawn_stops(&self, bot: &Bot, shown: &Controls, agents: Vec<SharedString>) -> Vec<SharedString> {
        let failed = matches!(self.about, About::Failed(_));
        let fixed = |stops: &[(bool, &'static str)]| -> Vec<SharedString> {
            stops.iter().filter(|(drawn, _)| *drawn).map(|(_, name)| SharedString::from(*name)).collect()
        };
        let before = fixed(&[(!bot.folder.is_empty(), COPY_STOP), (failed, RETRY_STOP)]);
        let after = fixed(&[(shown.routines, ROUTINES_STOP), (shown.edit, EDIT_STOP)]);
        [before, agents, after].concat()
    }

    fn keep_keys(&self, window: &mut Window) {
        let lost = self.stops.iter().any(|(name, handle)| handle.is_focused(window) && !self.drawn.contains(name));
        if lost {
            window.focus(&self.focus);
        }
    }

    fn walk(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let to = match press::key_of(&event.keystroke.key, event.keystroke.modifiers) {
            press::Key::Next => self.drawn.first(),
            press::Key::Previous => self.drawn.last(),
            press::Key::Press | press::Key::Other => return,
        };
        if let Some(handle) = to.and_then(|name| self.stops.get(name)) {
            window.focus(handle);
            cx.stop_propagation();
        }
    }

    fn card_stop(&self, element: Stateful<Div>, name: &str) -> Div {
        let Some(handle) = self.stops.get(name) else {
            return div().flex().child(press::stop(element, name.to_string(), true));
        };
        let line = div().flex().child(press::stop_at(element, handle, name.to_string(), true));
        if self.due.as_ref().is_some_and(|due| due.as_ref() == name) { revealed(line, &self.scroller) } else { line }
    }
}

fn made_day(bot: &Bot) -> Option<String> {
    bot.created.and_then(queue::local).map(|day| day.date())
}

fn tone_colour(tone: RowTone) -> gpui::Rgba {
    match tone {
        RowTone::Plain => colours::text(),
        RowTone::Danger => colours::failed(),
        RowTone::Waiting => colours::waiting(),
    }
}

fn labelled(label: &'static str, value: impl IntoElement) -> Div {
    div()
        .flex()
        .gap(px(10.))
        .text_size(px(12.5))
        .child(div().flex_none().w(px(LABEL_WIDTH)).text_color(colours::muted()).child(label))
        .child(div().flex_1().min_w_0().child(value))
}

fn fact_rows(rows: &[CardRow]) -> Div {
    let lines =
        rows.iter().map(|row| labelled(row.label, div().text_color(tone_colour(row.tone)).child(row.value.clone())));
    div().flex().flex_col().gap(px(6.)).children(lines)
}

fn folder_text(shown: &str) -> Div {
    let (parent, name) = folder_parts(shown);
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .text_size(px(12.5))
        .child(div().min_w_0().text_color(colours::muted()).child(ui::one_line(parent.to_string())))
        .child(div().flex_none().text_color(colours::text()).child(name.to_string()))
}

fn copy_pressed(event: &ClickEvent, (path, home): (&str, &Home), cx: &mut App) {
    if !press::pressed(event, COPY_STOP) {
        return;
    }
    Store::get(cx).update(cx, |s, cx| match home.machine() {
        None => s.copy(&Clip::of(path.to_string(), &ui::home_relative(path)), cx),
        Some(machine) => s.copy_from(path.to_string(), machine, cx),
    });
}

fn edit_pressed(event: &ClickEvent, bot: &str, window: &mut Window, cx: &mut App) {
    if press::pressed(event, EDIT_STOP) {
        edit_bot(bot, window, cx);
    }
}

fn routines_pressed(event: &ClickEvent, bot: &str, cx: &mut App) {
    if press::pressed(event, ROUTINES_STOP) {
        Store::get(cx).update(cx, |s, cx| {
            s.close_dialog(cx);
            s.open_routines(bot, cx);
        });
    }
}

fn section(title: &'static str, rows: Vec<AnyElement>, empty: &'static str, button: Option<AnyElement>) -> Div {
    let none = rows.is_empty().then(|| ui::dim(empty));
    ui::field(title, div().flex().flex_col().gap(px(8.)).children(rows).children(none).children(button))
}

fn listed(row: &ListRow, lead: Option<AnyElement>, action: Option<AnyElement>) -> Stateful<Div> {
    let lines = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .text_size(px(12.5))
        .child(div().text_color(colours::text()).child(ui::one_line(row.name.clone())))
        .child(ui::dim(ui::one_line(row.detail.clone())));
    div().id(("bot-card-row", row.id)).flex().items_center().gap(px(8.)).children(lead).child(lines).children(action)
}

impl BotCard {
    fn folder_line(&self, bot: &Bot) -> Option<Div> {
        if bot.folder.is_empty() {
            return None;
        }
        let (path, home) = (bot.folder.clone(), bot.home.clone());
        let copy =
            ui::button(COPY_STOP, "Copy path").on_click(move |event, _, cx| copy_pressed(event, (&path, &home), cx));
        let line = div()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(folder_text(&folder_shown(&bot.folder, &bot.home, &desktop::home_text())))
            .child(self.card_stop(copy, COPY_STOP));
        Some(labelled("Folder", line).items_center())
    }

    fn about_part(&self, cx: &mut Context<Self>) -> Option<Div> {
        let body = match &self.about {
            About::Hidden => return None,
            About::Lacks(words) => ui::dim(words.clone()),
            About::Reading => ui::dim(READING),
            About::Read(text) if text.trim().is_empty() => ui::dim(NOTHING_YET),
            About::Read(text) => div().text_size(px(12.5)).text_color(colours::text()).child(text.clone()),
            About::Failed(words) => self.about_failed(words, cx),
        };
        Some(ui::field(FOR, body))
    }

    fn about_failed(&self, words: &str, cx: &mut Context<Self>) -> Div {
        let again = ui::button(RETRY_STOP, TRY_AGAIN).on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
            if press::pressed(event, RETRY_STOP) {
                this.read_about(cx);
            }
        }));
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(ui::error_line(words.to_string()))
            .child(self.card_stop(again, RETRY_STOP))
    }

    fn routines_part(&self, rows: &[ListRow], button: bool) -> Div {
        let bot = self.bot.id.clone();
        let open = button.then(|| {
            let button =
                ui::button(ROUTINES_STOP, "Routines…").on_click(move |event, _, cx| routines_pressed(event, &bot, cx));
            self.card_stop(button, ROUTINES_STOP).into_any_element()
        });
        let lines = rows.iter().map(|row| listed(row, None, None).into_any_element()).collect();
        section(ROUTINES_TITLE, lines, NO_ROUTINES, open)
    }

    fn body(&self, bot: &Bot, (agents, shown): (Option<&[StartedRow]>, &Controls), cx: &mut Context<Self>) -> Div {
        let store = Store::get(cx).read(cx);
        let now = queue::now();
        let routines = store
            .routines_offered_at(&bot.home)
            .then(|| store.bots_at(&bot.home).map(|bots| routine_rows(&bots.queue, &bot.id, |at| time_label(at, now))))
            .flatten();
        let reach = bot.home.machine().and_then(|machine| store.machine_bots_note(machine));
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .children(reach.map(ui::dim))
            .children(self.folder_line(bot))
            .child(fact_rows(&card_rows(bot, made_day(bot))))
            .children(self.about_part(cx))
            .children(agents.map(|rows| self.started_part(rows)))
            .children(routines.map(|rows| self.routines_part(&rows, shown.routines)))
            .children(shown.note.clone().map(ui::dim))
    }

    fn buttons(&self, shown: &Controls) -> Div {
        let bot = self.bot.id.clone();
        let edit = shown.edit.then(|| {
            let button = ui::button(EDIT_STOP, "Edit bot…")
                .on_click(move |event: &ClickEvent, window, cx| edit_pressed(event, &bot, window, cx));
            self.card_stop(button, EDIT_STOP)
        });
        let done = ui::primary_button("bot-card-done", "Done").on_click(|_: &ClickEvent, _, cx| close(cx));
        div().flex().gap(px(8.)).children(edit).child(done)
    }
}

impl Focusable for BotCard {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for BotCard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.reveal(window);
        let store = Store::get(cx).read(cx);
        let Some(bot) = store.bot(&self.bot).cloned() else { return div() };
        let (shown, agents) = (controls(&bot.home, store.routines_offered_at(&bot.home)), started(store, &self.bot));
        let rows = agents.as_deref().unwrap_or_default();
        self.keep_started_stops(rows, cx);
        self.drawn = self.drawn_stops(&bot, &shown, started_stops(rows));
        self.keep_keys(window);
        let height = body_height(f32::from(window.viewport_size().height));
        let body = self.body(&bot, (agents.as_deref(), &shown), cx);
        let column = div()
            .flex()
            .flex_col()
            .max_h(px(height))
            .child(fitted("bot-card", window, body).min_h_0().track_scroll(&self.scroller));
        div().track_focus(&self.focus).on_key_down(cx.listener(Self::walk)).child(frame(
            card_title(&bot.label()),
            column,
            self.buttons(&shown),
        ))
    }
}
