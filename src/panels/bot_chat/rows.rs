use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gpui::{
    AnyElement, ClickEvent, Div, FocusHandle, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _, WeakEntity, div, px, relative,
};

use super::BotChat;
use super::entries::{Entry, Retry, folded};
use super::words::{DROPPED_STAMP, EARLIER, HELD, RETRY, SENDING, folded_label, time_label};
use crate::model::bots::Said;
use crate::store::Store;
use crate::theme::{FONT_FAMILY, colours, radius};
use crate::ui::markdown::text as md;
use crate::ui::markdown::{self, Block};
use crate::ui::{self, Tone, glass, press};

const RETRY_STOP: &str = "bot-retry";

pub(super) type Blocks = Rc<RefCell<HashMap<u64, Rc<Vec<Block>>>>>;

#[derive(Clone)]
pub(super) struct Meta {
    pub name: SharedString,
    pub now: i64,
    pub chat: WeakEntity<BotChat>,
    pub opened: Rc<HashSet<u64>>,
    pub blocks: Blocks,
    pub retry: Option<Rc<Retry>>,
    pub bot: String,
    pub sending: bool,
    pub stops: bool,
    pub retry_stop: FocusHandle,
}

pub(super) fn earlier_row() -> AnyElement {
    line(div().text_size(px(11.)).text_color(colours::faint()).text_center().child(EARLIER)).into_any_element()
}

fn line(inner: Div) -> Div {
    div().w_full().flex().justify_center().px(px(16.)).py(px(5.)).child(div().w_full().max_w(px(820.)).child(inner))
}

pub(super) fn entry_row(entry: &Entry, meta: &Meta) -> AnyElement {
    match entry {
        Entry::You { text, at, held, dropped, .. } => you(text, *at, (*held, *dropped), meta),
        Entry::Answer { id, text, at } => answer(*id, text, *at, meta),
        Entry::Steps { first, texts, .. } => steps(*first, texts, meta),
        Entry::Failed { id, text } => failed(*id, text, meta),
        Entry::Notice { text, .. } => notice(text),
        Entry::Other { text, .. } => plain(text),
    }
    .into_any_element()
}

fn you(text: &str, at: i64, (held, dropped): (bool, bool), meta: &Meta) -> Div {
    let stamp = match (dropped, held) {
        (true, _) => DROPPED_STAMP.to_string(),
        (false, true) => HELD.to_string(),
        (false, false) => time_label(at, meta.now),
    };
    let bubble = div()
        .max_w(relative(0.8))
        .px(px(12.))
        .py(px(8.))
        .rounded(px(radius::card()))
        .bg(glass::tint(colours::brand(), 0.22))
        .text_color(colours::text())
        .child(text.to_string());
    let stamp = div().text_size(px(10.5)).text_color(colours::faint()).child(stamp);
    let bubble = if dropped { bubble.opacity(0.5) } else { bubble };
    line(div().flex().flex_col().items_end().gap(px(3.)).child(bubble).child(stamp))
}

fn blocks_of(id: u64, text: &str, meta: &Meta) -> Rc<Vec<Block>> {
    meta.blocks.borrow_mut().entry(id).or_insert_with(|| Rc::new(markdown::parse(text))).clone()
}

fn answer(id: u64, text: &str, at: i64, meta: &Meta) -> Div {
    let blocks = blocks_of(id, text, meta);
    let head = div()
        .flex()
        .items_center()
        .gap(px(4.))
        .text_size(px(11.))
        .text_color(colours::faint())
        .child(div().font_weight(FontWeight::SEMIBOLD).child(format!("{} · {}", meta.name, time_label(at, meta.now))))
        .child(ui::copy_button(("bot-answer", id), text.to_string()));
    let body = div().flex().flex_col().gap(px(6.)).text_color(colours::soft()).children(md::linked_blocks(
        &format!("bot-{id}"),
        &blocks,
        &md::opens_web_addresses(),
    ));
    line(div().flex().flex_col().gap(px(2.)).child(head).child(body))
}

fn steps(first: u64, texts: &[String], meta: &Meta) -> Div {
    let (hidden, shown) = folded(texts, meta.opened.contains(&first));
    let more = (hidden > 0).then(|| {
        let chat = meta.chat.clone();
        div()
            .id(("bot-steps", first))
            .cursor_pointer()
            .text_color(colours::muted())
            .rounded(px(radius::chip()))
            .px(px(4.))
            .ml(px(-4.))
            .hover(|s| s.bg(colours::hover()).text_color(colours::soft()))
            .child(folded_label(hidden))
            .on_click(move |_: &ClickEvent, _, cx| {
                let _ = chat.update(cx, |chat, cx| chat.open_steps(first, cx));
            })
    });
    let rows = shown.iter().map(|text| div().child(ui::one_line(format!("▸ {text}"))));
    line(
        div()
            .flex()
            .flex_col()
            .text_size(px(11.5))
            .text_color(colours::faint())
            .font_family(FONT_FAMILY)
            .children(more)
            .children(rows),
    )
}

fn failed(id: u64, text: &str, meta: &Meta) -> Div {
    let retry = meta.retry.as_ref().filter(|retry| retry.error == id).map(|retry| retry_button(retry, meta));
    let text = div().text_color(colours::failed()).child(text.to_string());
    line(div().flex().flex_col().items_start().gap(px(6.)).child(text).children(retry))
}

fn retry_button(retry: &Retry, meta: &Meta) -> AnyElement {
    if meta.sending {
        return ui::busy_button("bot-retry", SENDING, Tone::Plain).into_any_element();
    }
    let (bot, text) = (meta.bot.clone(), retry.text.clone());
    press::stop_at(ui::button(RETRY_STOP, RETRY), &meta.retry_stop, RETRY_STOP, meta.stops)
        .on_click(move |event: &ClickEvent, _, cx| {
            if press::pressed(event, RETRY_STOP) {
                let (bot, text) = (bot.clone(), text.clone());
                Store::get(cx).update(cx, |store, cx| store.say_to_bot(&bot, text, Said::Again, cx));
            }
        })
        .into_any_element()
}

fn notice(text: &str) -> Div {
    line(div().text_size(px(11.5)).text_color(colours::muted()).text_center().child(text.to_string()))
}

fn plain(text: &str) -> Div {
    line(div().text_color(colours::muted()).child(text.to_string()))
}

pub(super) fn chip(chat: WeakEntity<BotChat>) -> impl IntoElement {
    div().absolute().bottom(px(10.)).right(px(18.)).child(
        div()
            .id("bot-new-messages")
            .px(px(10.))
            .py(px(4.))
            .rounded_full()
            .cursor_pointer()
            .bg(colours::brand())
            .text_color(colours::on_brand())
            .text_size(px(11.5))
            .child(super::words::NEW_MESSAGES)
            .on_click(move |_: &ClickEvent, _, cx| {
                let _ = chat.update(cx, BotChat::scroll_to_end);
            }),
    )
}
