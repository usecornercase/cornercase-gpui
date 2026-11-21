use gpui::{
    App, AppContext as _, ClickEvent, Context, Div, Entity, FocusHandle, Focusable, IntoElement, ParentElement as _,
    Render, ScrollHandle, Stateful, StatefulInteractiveElement as _, Styled as _, Window, div,
    prelude::FluentBuilder as _, px,
};

use super::{
    body_height, bot_refusal, close, dialog_starts, fitted, forget_refusal, frame, hooked_field, secret_label,
    show_at_top, within,
};
use crate::cli;
use crate::model::bots::{
    Ask, Bot, Enter, IT_SAYS, ONE_LINE, access_words, fields_after, give_json, on_enter, title, what_happens,
};
use crate::store::Store;
use crate::theme::colours;
use crate::ui::{self, TextField, Tone, press};

const NEVER: &str = "Never";
const NOT_NOW: &str = "Not now";
const SAVE: &str = "Save";
const SAVING: &str = "Saving…";
const NEVER_STOP: &str = "bot-ask-never";

pub(crate) struct BotAsk {
    bot: String,
    ask: u64,
    fields: Vec<(String, Entity<TextField>)>,
    note: Option<&'static str>,
    scroller: ScrollHandle,
}

pub(crate) fn answer_ask(bot: &Bot, ask: u64, window: &mut Window, cx: &mut App) {
    let Some(found) = bot.asks.iter().find(|found| found.id == ask) else { return };
    let names = found.fields().to_vec();
    let view = cx.new(|cx| BotAsk::new((&bot.id, ask), &names, window, cx));
    let Some(first) = view.read(cx).fields.first().map(|(_, field)| field.clone()) else { return };
    show_at_top(view.into(), &first, window, cx);
    Store::get(cx).update(cx, |s, _| {
        s.names_bot(&bot.key());
        s.names_ask(&bot.id, ask);
    });
}

fn kept_or_new(
    name: String,
    old: &mut Vec<(String, Entity<TextField>)>,
    scroller: &ScrollHandle,
    window: &mut Window,
    cx: &mut Context<BotAsk>,
) -> (String, Entity<TextField>) {
    match old.iter().position(|(had, _)| *had == name) {
        Some(at) => old.swap_remove(at),
        None => (name, BotAsk::field(scroller, window, cx)),
    }
}

impl BotAsk {
    fn new((bot, ask): (&str, u64), names: &[String], window: &mut Window, cx: &mut Context<Self>) -> Self {
        let scroller = ScrollHandle::new();
        let fields = names.iter().map(|name| (name.clone(), Self::field(&scroller, window, cx))).collect();
        cx.observe_in(&Store::get(cx), window, |this, _, window, cx| this.follow(window, cx)).detach();
        let this = Self { bot: bot.to_string(), ask, fields, note: None, scroller };
        this.mark_end(cx);
        this
    }

    fn field(scroller: &ScrollHandle, window: &mut Window, cx: &mut Context<Self>) -> Entity<TextField> {
        let me = cx.entity().downgrade();
        let shape = within(scroller, move |field: TextField| {
            field.masked().one_value(move |_, cx| {
                let _ = me.update(cx, Self::refused_paste);
            })
        });
        hooked_field("", shape, cx, (Self::submit, Self::changed), window)
    }

    fn live(&self, cx: &App) -> Option<Ask> {
        let store = Store::get(cx).read(cx);
        store.bots.find(&self.bot)?.asks.iter().find(|ask| ask.id == self.ask).cloned()
    }

    fn follow(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ask) = self.live(cx) {
            let have: Vec<String> = self.fields.iter().map(|(name, _)| name.clone()).collect();
            let names = fields_after(&have, &ask);
            if names != have {
                let (mut old, scroller) = (std::mem::take(&mut self.fields), self.scroller.clone());
                self.fields =
                    names.into_iter().map(|name| kept_or_new(name, &mut old, &scroller, window, cx)).collect();
                self.mark_end(cx);
            }
        }
        cx.notify();
    }

    fn mark_end(&self, cx: &mut App) {
        let last = self.fields.len().saturating_sub(1);
        for (at, (_, field)) in self.fields.iter().enumerate() {
            field.update(cx, |field, _| field.set_ends_form(at == last));
        }
    }

    fn refused_paste(&mut self, cx: &mut Context<Self>) {
        self.note = Some(ONE_LINE);
        cx.notify();
    }

    fn changed(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.note = None;
        forget_refusal(cx);
        cx.notify();
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let empty: Vec<bool> = self.fields.iter().map(|(_, field)| field.read(cx).text().is_empty()).collect();
        match on_enter(&empty) {
            Enter::Save => self.save(cx),
            Enter::Focus(at) => {
                if let Some((_, field)) = self.fields.get(at) {
                    window.focus(&field.focus_handle(cx));
                }
            }
        }
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        if dialog_starts(cx) {
            return;
        }
        let Some(ask) = self.live(cx) else { return };
        let typed: Vec<(String, String)> =
            self.fields.iter().map(|(name, field)| (name.clone(), field.read(cx).text())).collect();
        let Some(json) = give_json(&ask, &typed) else { return };
        for (_, field) in &self.fields {
            field.update(cx, TextField::clear);
        }
        let (bot, ask, dialog) = (self.bot.clone(), self.ask, cx.entity_id().as_u64());
        Store::get(cx).update(cx, |s, cx| s.give_secrets((&bot, ask), json, dialog, cx));
    }

    fn never(&mut self, cx: &mut Context<Self>) {
        let bot = self.bot.clone();
        let ask = self.ask;
        Store::get(cx).update(cx, |s, cx| s.decline_ask((&bot, ask), cx));
        close(cx);
    }
}

fn agent_line(bot: &Bot) -> Div {
    let line = div().flex().gap(px(4.)).text_size(px(12.)).text_color(colours::muted()).child(bot.agent.clone());
    match access_words(&bot.access) {
        None => line,
        Some((words, full)) => {
            line.child("·").child(div().when(full, |d| d.text_color(colours::failed())).child(words))
        }
    }
}

fn said(about: &str) -> Div {
    let words = div().pl(px(10.)).border_l_2().border_color(colours::border()).text_color(colours::soft());
    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .text_size(px(12.5))
        .child(div().text_color(colours::muted()).child(IT_SAYS))
        .child(words.child(about.to_string()))
}

impl BotAsk {
    fn fields_column(&self, replace: bool, window: &Window, cx: &App) -> Div {
        let rows = self.fields.iter().map(|(name, field)| {
            div().flex().flex_col().gap(px(6.)).child(secret_label(name, replace)).child(ui::boxed(field, window, cx))
        });
        div().flex().flex_col().gap(px(10.)).children(rows)
    }

    fn body(&self, (bot, ask): (&Bot, &Ask), window: &Window, cx: &App) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(agent_line(bot))
            .child(said(&ask.about))
            .child(self.fields_column(ask.replace, window, cx))
            .child(ui::dim(what_happens(cli::host().is_none())))
    }

    fn never_button(starts: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        if starts {
            return ui::resting_button(NEVER_STOP, NEVER, Tone::Plain);
        }
        press::stop(ui::button(NEVER_STOP, NEVER), NEVER_STOP, true).on_click(cx.listener(
            |this, event: &ClickEvent, _, cx| {
                if press::pressed(event, NEVER_STOP) {
                    this.never(cx);
                }
            },
        ))
    }

    fn buttons(cx: &mut Context<Self>) -> Div {
        let starts = dialog_starts(cx);
        let later = ui::button("bot-ask-later", NOT_NOW).on_click(|_: &ClickEvent, _, cx| close(cx));
        let save = if starts {
            ui::busy_button("bot-ask-save", SAVING, Tone::Primary)
        } else {
            ui::primary_button("bot-ask-save", SAVE)
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.submit(window, cx)))
        };
        div().flex().gap(px(8.)).child(Self::never_button(starts, cx)).child(later).child(save)
    }
}

impl Focusable for BotAsk {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        match self.fields.first() {
            Some((_, field)) => field.read(cx).focus_handle(cx),
            None => cx.focus_handle(),
        }
    }
}

impl Render for BotAsk {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let store = Store::get(cx).read(cx);
        let Some(bot) = store.bots.find(&self.bot).cloned() else { return div() };
        let Some(ask) = bot.asks.iter().find(|ask| ask.id == self.ask).cloned() else { return div() };
        let height = body_height(f32::from(window.viewport_size().height));
        let body = self.body((&bot, &ask), window, cx);
        let column = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .max_h(px(height))
            .child(fitted("bot-ask", window, body).min_h_0().track_scroll(&self.scroller))
            .children(bot_refusal(self.note, cx.entity_id().as_u64(), cx));
        frame(title(&bot.name), column, Self::buttons(cx))
    }
}
