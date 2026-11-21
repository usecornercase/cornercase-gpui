use gpui::{
    App, AppContext as _, ClickEvent, Context, Div, Entity, FocusHandle, Focusable, IntoElement, ParentElement as _,
    Render, ScrollHandle, Stateful, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use super::{
    body_height, bot_refusal, cancel_and, dialog_starts, fitted, forget_refusal, frame, hooked_field, muted,
    secret_label, within,
};
use crate::cli;
use crate::model::bots::{
    NAME_HINT, NEW_SECRET, ONE_LINE, SECRET_VALUE, SecretEnter, on_secret_enter, replaces, what_a_secret_is,
};
use crate::store::Store;
use crate::ui::{self, TextField, Tone};

const SAVE: &str = "Save";
const SAVING: &str = "Saving…";
const SAVE_ID: &str = "bot-secret-save";

type Hook = fn(&mut BotSecret, &mut Window, &mut Context<BotSecret>);

enum Named {
    Typed(Entity<TextField>),
    Fixed(String),
}

pub(crate) struct BotSecret {
    bot: String,
    name: Named,
    value: Entity<TextField>,
    saved: Vec<String>,
    note: Option<String>,
    scroller: ScrollHandle,
}

pub(super) struct Opening {
    pub bot: String,
    pub name: Option<String>,
    pub saved: Vec<String>,
    pub back: FocusHandle,
}

pub(super) fn open_secret(opening: Opening, window: &mut Window, cx: &mut App) {
    let Opening { bot, name, saved, back } = opening;
    let view = cx.new(|cx| BotSecret::new((bot, name), saved, window, cx));
    let focus = view.read(cx).focus_handle(cx);
    Store::get(cx).update(cx, |s, cx| s.show_dialog_over(view.into(), focus, back, cx));
}

impl BotSecret {
    fn new(
        (bot, name): (String, Option<String>),
        saved: Vec<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let scroller = ScrollHandle::new();
        let name = match name {
            Some(name) => Named::Fixed(name),
            None => Named::Typed(hooked_field(NAME_HINT, within(&scroller, |f| f), cx, Self::hooks(), window)),
        };
        let me = cx.entity().downgrade();
        let shape = within(&scroller, move |field: TextField| {
            field.masked().one_value(move |_, cx| {
                let _ = me.update(cx, |this, cx| this.say(ONE_LINE.to_string(), cx));
            })
        });
        let value = hooked_field("", shape, cx, Self::hooks(), window);
        value.update(cx, |field, _| field.set_ends_form(true));
        cx.observe(&Store::get(cx), |_, _, cx| cx.notify()).detach();
        Self { bot, name, value, saved, note: None, scroller }
    }

    fn hooks() -> (Hook, Hook) {
        (Self::submit, Self::changed)
    }

    fn changed(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.note = None;
        forget_refusal(cx);
        cx.notify();
    }

    fn say(&mut self, note: String, cx: &mut Context<Self>) {
        self.note = Some(note);
        cx.notify();
    }

    fn typed_name(&self, cx: &App) -> String {
        match &self.name {
            Named::Typed(field) => field.read(cx).text().trim().to_string(),
            Named::Fixed(name) => name.clone(),
        }
    }

    fn focus_name(&self, window: &mut Window, cx: &App) {
        window.focus(&self.focus_handle(cx));
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if dialog_starts(cx) {
            return;
        }
        let name = self.typed_name(cx);
        match on_secret_enter(&name, self.value.read(cx).text().is_empty()) {
            SecretEnter::FocusName => self.focus_name(window, cx),
            SecretEnter::Refused(words) => {
                self.say(words, cx);
                self.focus_name(window, cx);
            }
            SecretEnter::FocusValue => window.focus(&self.value.focus_handle(cx)),
            SecretEnter::Save => self.save(&name, cx),
        }
    }

    fn save(&mut self, name: &str, cx: &mut Context<Self>) {
        let value = self.value.read(cx).text();
        self.value.update(cx, TextField::clear);
        let (bot, dialog) = (self.bot.clone(), cx.entity_id().as_u64());
        Store::get(cx).update(cx, |s, cx| s.save_secret((&bot, name), value, dialog, cx));
    }
}

fn labelled(label: Div, control: Div) -> Div {
    div().flex().flex_col().gap(px(6.)).child(label).child(control)
}

impl BotSecret {
    fn title(&self) -> &'static str {
        match self.name {
            Named::Typed(_) => NEW_SECRET,
            Named::Fixed(_) => SECRET_VALUE,
        }
    }

    fn head(&self, cx: &App) -> String {
        let store = Store::get(cx).read(cx);
        let bot = store.bots.find(&self.bot).map_or_else(|| self.bot.clone(), |found| found.name.clone());
        match &self.name {
            Named::Typed(_) => bot,
            Named::Fixed(name) => format!("{bot} · {name}"),
        }
    }

    fn name_row(&self, window: &Window, cx: &App) -> Option<Div> {
        let Named::Typed(field) = &self.name else { return None };
        let replace = replaces(field.read(cx).text().trim(), &self.saved);
        Some(labelled(secret_label("Name", replace), ui::boxed(field, window, cx)))
    }

    fn body(&self, window: &Window, cx: &App) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(muted(self.head(cx)))
            .children(self.name_row(window, cx))
            .child(labelled(secret_label("Value", false), ui::boxed(&self.value, window, cx)))
            .child(ui::dim(what_a_secret_is(cli::host().is_none())))
    }

    fn save_button(cx: &mut Context<Self>) -> Stateful<Div> {
        if dialog_starts(cx) {
            return ui::busy_button(SAVE_ID, SAVING, Tone::Primary);
        }
        ui::primary_button(SAVE_ID, SAVE)
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.submit(window, cx)))
    }
}

impl Focusable for BotSecret {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        match &self.name {
            Named::Typed(field) => field.read(cx).focus_handle(cx),
            Named::Fixed(_) => self.value.read(cx).focus_handle(cx),
        }
    }
}

impl Render for BotSecret {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let height = body_height(f32::from(window.viewport_size().height));
        let body = self.body(window, cx);
        let column = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .max_h(px(height))
            .child(fitted("bot-secret", window, body).min_h_0().track_scroll(&self.scroller))
            .children(bot_refusal(self.note.as_deref(), cx.entity_id().as_u64(), cx));
        frame(self.title(), column, cancel_and(Self::save_button(cx)))
    }
}
