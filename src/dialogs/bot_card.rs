use std::collections::HashMap;

use gpui::{
    AnyElement, App, AppContext as _, ClickEvent, Context, Div, FocusHandle, Focusable, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, ScrollHandle, SharedString, Stateful, StatefulInteractiveElement as _,
    Styled as _, Window, div, px,
};

use super::{body_height, close, edit_bot, fitted, frame, revealed, show};
use crate::cli;
use crate::model::bots::{
    AboutGate, Bot, BotKey, CardRow, ListRow, Tone as RowTone, about_args, about_failed, about_of, card_rows,
    routine_rows,
};
use crate::panels::bot_chat::time_label;
use crate::panels::queue;
use crate::store::{self, Busy, Clip, Store};
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
    bot: String,
    about: About,
    serial: u64,
    focus: FocusHandle,
    stops: HashMap<SharedString, FocusHandle>,
    keyed: Option<SharedString>,
    due: Option<SharedString>,
    scroller: ScrollHandle,
}

pub(crate) fn bot_card(id: &str, window: &mut Window, cx: &mut App) {
    if Store::get(cx).read(cx).bots.find(id).is_none() {
        return;
    }
    let view = cx.new(|cx| BotCard::new(id, cx));
    let focus = view.read(cx).focus.clone();
    show(view.into(), &focus, window, cx);
    Store::get(cx).update(cx, |s, _| s.names_bot(&BotKey::here(id)));
}

impl BotCard {
    fn new(id: &str, cx: &mut Context<Self>) -> Self {
        cx.observe(&Store::get(cx), |_, _, cx| cx.notify()).detach();
        let about = match Store::get(cx).read(cx).about_gate() {
            AboutGate::Hidden => About::Hidden,
            AboutGate::Lacks(words) => About::Lacks(words),
            AboutGate::Read => About::Reading,
        };
        let mut card = Self {
            bot: id.to_string(),
            about,
            serial: 0,
            focus: cx.focus_handle(),
            stops: STOPS.iter().map(|name| (SharedString::from(*name), cx.focus_handle())).collect(),
            keyed: None,
            due: None,
            scroller: ScrollHandle::new(),
        };
        if matches!(card.about, About::Reading) {
            card.read_about(cx);
        }
        card
    }

    fn read_about(&mut self, cx: &mut Context<Self>) {
        let name = Store::get(cx).read(cx).bots.find(&self.bot).map_or_else(|| self.bot.clone(), |b| b.name.clone());
        self.serial += 1;
        self.about = About::Reading;
        let serial = self.serial;
        let busy = Busy::new(format!("bot-about:{}", self.bot), format!("Reading what {name} is for…"));
        store::run_busy(cx, busy, about_args(&self.bot), move |this, result, cx| this.about_read(serial, result, cx));
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

fn folder_text(path: &str) -> Div {
    let shown = ui::home_relative(path);
    let (parent, name) = shown.rsplit_once('/').map_or((String::new(), shown.as_str()), |(p, n)| (format!("{p}/"), n));
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .text_size(px(12.5))
        .child(div().min_w_0().text_color(colours::muted()).child(ui::one_line(parent)))
        .child(div().flex_none().text_color(colours::text()).child(name.to_string()))
}

fn copy_pressed(event: &ClickEvent, path: &str, cx: &mut App) {
    if press::pressed(event, COPY_STOP) {
        let clip = Clip::of(path.to_string(), &ui::home_relative(path));
        Store::get(cx).update(cx, |s, cx| s.copy(&clip, cx));
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
        let path = bot.folder.clone();
        let copy = ui::button(COPY_STOP, "Copy path").on_click(move |event, _, cx| copy_pressed(event, &path, cx));
        let line = div()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(folder_text(&bot.folder))
            .child(self.card_stop(copy, COPY_STOP));
        Some(labelled("Folder", line))
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

    fn routines_part(&self, rows: &[ListRow]) -> Div {
        let bot = self.bot.clone();
        let open =
            ui::button(ROUTINES_STOP, "Routines…").on_click(move |event, _, cx| routines_pressed(event, &bot, cx));
        let lines = rows.iter().map(|row| listed(row, None, None).into_any_element()).collect();
        section(ROUTINES_TITLE, lines, NO_ROUTINES, Some(self.card_stop(open, ROUTINES_STOP).into_any_element()))
    }

    fn body(&self, bot: &Bot, cx: &mut Context<Self>) -> Div {
        let store = Store::get(cx).read(cx);
        let now = queue::now();
        let routines = store
            .routines_offered()
            .then(|| routine_rows(&queue::entries(store.app.as_ref()), &bot.id, |at| time_label(at, now)));
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .children(self.folder_line(bot))
            .child(fact_rows(&card_rows(bot, made_day(bot))))
            .children(self.about_part(cx))
            .children(routines.map(|rows| self.routines_part(&rows)))
    }

    fn buttons(&self) -> Div {
        let bot = self.bot.clone();
        let edit = ui::button(EDIT_STOP, "Edit bot…").on_click(move |event: &ClickEvent, window, cx| {
            if press::pressed(event, EDIT_STOP) {
                edit_bot(&bot, window, cx);
            }
        });
        let done = ui::primary_button("bot-card-done", "Done").on_click(|_: &ClickEvent, _, cx| close(cx));
        div().flex().gap(px(8.)).child(self.card_stop(edit, EDIT_STOP)).child(done)
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
        let Some(bot) = Store::get(cx).read(cx).bots.find(&self.bot).cloned() else { return div() };
        let height = body_height(f32::from(window.viewport_size().height));
        let body = self.body(&bot, cx);
        let column = div()
            .flex()
            .flex_col()
            .max_h(px(height))
            .child(fitted("bot-card", window, body).min_h_0().track_scroll(&self.scroller));
        div().track_focus(&self.focus).child(frame(card_title(&bot.name), column, self.buttons()))
    }
}
