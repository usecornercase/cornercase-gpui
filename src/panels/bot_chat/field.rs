use gpui::{
    AppContext as _, Context, Entity, FocusHandle, Focusable as _, IntoElement, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _, Window, div, px,
};

use super::entries::{Control, send_control};
use super::words::{SEND, SENDING, placeholder};
use crate::model::bots::{Said, on_return, say_key, sendable};
use crate::store::Store;
use crate::theme::colours;
use crate::ui::{self, TextField, TextFieldEvent, Tone};

pub(crate) struct ChatField {
    pub(super) store: Entity<Store>,
    field: Entity<TextField>,
    owner: Option<String>,
    seen: (String, u64),
}

impl ChatField {
    pub(super) fn new(store: Entity<Store>, cx: &mut Context<Self>) -> Self {
        let field = cx.new(|cx| TextField::new("", cx).chat());
        cx.subscribe(&field, |this, _, event: &TextFieldEvent, cx| match event {
            TextFieldEvent::Changed => this.keep_draft(cx),
            TextFieldEvent::Submit => this.send(cx),
            TextFieldEvent::Cancel => {}
        })
        .detach();
        cx.observe(&store, |this, _, cx| this.follow(cx)).detach();
        let mut this = Self { store, field, owner: None, seen: (String::new(), 0) };
        this.follow(cx);
        this
    }

    pub(super) fn focus_handle(&self, cx: &gpui::App) -> FocusHandle {
        self.field.read(cx).focus_handle(cx)
    }

    fn follow(&mut self, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let Some(shown) = store.shown_bot().map(str::to_string) else { return };
        let name = store.bots.find(&shown).map_or_else(|| shown.clone(), |bot| bot.name.clone());
        let revision = store.bots.drafts.revision();
        let draft = store.bots.drafts.text(&shown).to_string();
        let changed_bot = self.owner.as_deref() != Some(shown.as_str());
        let again = !changed_bot && self.seen.1 != revision;
        if self.seen.0 != name {
            let label = placeholder(&name);
            self.field.update(cx, |field, cx| field.set_placeholder(label, cx));
        }
        self.seen = (name, revision);
        if changed_bot || again {
            self.owner = Some(shown);
            self.field.update(cx, |field, cx| field.fill(draft, cx));
        }
        cx.notify();
    }

    fn keep_draft(&mut self, cx: &mut Context<Self>) {
        if let Some(owner) = self.owner.clone() {
            let text = self.field.read(cx).text();
            self.store.update(cx, |store, _| store.keep_draft(&owner, text));
        }
        cx.notify();
    }

    fn send(&mut self, cx: &mut Context<Self>) {
        let Some(owner) = self.owner.clone() else { return };
        let on_its_way = self.store.read(cx).busy(&say_key(&owner));
        let Some(text) = on_return(&self.field.read(cx).text(), on_its_way) else { return };
        self.field.update(cx, TextField::clear);
        self.store.update(cx, |store, cx| store.say_to_bot(&owner, text, Said::Typed, cx));
        super::global::to_end(cx);
    }

    fn send_button(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let blank = !sendable(&self.field.read(cx).text());
        let on_its_way = self.owner.as_deref().is_some_and(|bot| self.store.read(cx).busy(&say_key(bot)));
        let width = div().w(px(86.)).flex_none();
        match send_control(blank, on_its_way) {
            Control::Running => {
                width.child(ui::busy_button("bot-send", SENDING, Tone::Primary).w_full().justify_center())
            }
            Control::Resting => width.child(ui::resting_button("bot-send", SEND, Tone::Primary).w_full().text_center()),
            Control::Ready => width.child(
                ui::primary_button("bot-send", SEND)
                    .w_full()
                    .text_center()
                    .on_click(cx.listener(|this, _, _, cx| this.send(cx))),
            ),
        }
    }
}

impl Render for ChatField {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let field = ui::boxed(&self.field, window, cx).flex_1().min_w_0();
        div()
            .flex_none()
            .flex()
            .items_end()
            .gap(px(8.))
            .px(px(16.))
            .py(px(10.))
            .border_t_1()
            .border_color(colours::border())
            .child(field)
            .child(self.send_button(cx))
    }
}
