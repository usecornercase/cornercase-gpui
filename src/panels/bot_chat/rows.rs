use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, Div, FocusHandle, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, WeakEntity, div, px, relative,
};

use super::BotChat;
use super::agent_row::agent;
use super::agents::Stop;
use super::entries::{Again, Entry, Retry, STEPS_SHOWN, folded};
use super::picking::Spot;
use super::run_block::run;
use super::words::{DROPPED_STAMP, EARLIER, GIVE, HELD, RETRY, SENDING, SETTING_UP, folded_label, time_label};
use crate::model::bots::{BotKey, Give, Runs, Said};
use crate::store::{Clip, Store};
use crate::theme::{FONT_FAMILY, colours, radius};
use crate::ui::markdown::text as md;
use crate::ui::markdown::{self, Block};
use crate::ui::selection::{self, Laid, Lit, Selection};
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
    pub bot: BotKey,
    pub sending: bool,
    pub stops: bool,
    pub retry_stop: FocusHandle,
    pub givable: Rc<HashMap<u64, u64>>,
    pub giving: Give,
    pub runs: Rc<Runs>,
    pub again_busy: Rc<HashSet<u64>>,
    pub again_stops: Rc<HashMap<u64, FocusHandle>>,
    pub laid: Laid<Spot>,
    pub selection: Option<Selection<Spot>>,
    pub on_link: md::OnLink,
    pub agent_stops: Rc<HashMap<Stop, FocusHandle>>,
}

impl Meta {
    fn pick(&self, spot: Spot, text: &str) -> selection::Pick {
        let lit = self.selection.map_or(Lit::Nothing, |selection| selection.lit(spot, text));
        selection::pick(&self.laid, spot, lit)
    }

    fn picked(&self, spot: Spot, text: &str) -> Div {
        self.pick(spot, text).draw(text.to_string(), Vec::new(), IntoElement::into_any_element)
    }
}

pub(super) fn earlier_row() -> AnyElement {
    line(div().text_size(px(11.)).text_color(colours::faint()).text_center().child(EARLIER)).into_any_element()
}

pub(super) fn line(inner: Div) -> Div {
    div().w_full().flex().justify_center().px(px(16.)).py(px(5.)).child(div().w_full().max_w(px(820.)).child(inner))
}

pub(super) fn entry_row(entry: &Entry, meta: &Meta) -> AnyElement {
    match entry {
        Entry::You { id, text, at, held, dropped } => you((*id, text), *at, (*held, *dropped), meta),
        Entry::Answer { id, text, at } => line(answer((*id, *id), text, *at, meta)),
        Entry::Steps { first, texts, .. } => line(steps(*first, texts, STEPS_SHOWN, meta)),
        Entry::Failed { id, text } => failed(*id, text, meta),
        Entry::Notice { id, text, .. } => notice(*id, text, meta),
        Entry::Other { text, .. } => plain(text),
        Entry::Agent(line) => agent(line, meta),
        Entry::Run(block) => run(block, meta),
    }
    .into_any_element()
}

fn you((id, text): (u64, &str), at: i64, (held, dropped): (bool, bool), meta: &Meta) -> Div {
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
        .child(meta.picked(Spot { entry: id, part: 0 }, text));
    let stamp = div().text_size(px(10.5)).text_color(colours::faint()).child(stamp);
    let bubble = if dropped { bubble.opacity(0.5) } else { bubble };
    line(div().flex().flex_col().items_end().gap(px(3.)).child(bubble).child(stamp))
}

pub(super) fn blocks_of(id: u64, text: &str, blocks: &Blocks) -> Rc<Vec<Block>> {
    blocks.borrow_mut().entry(id).or_insert_with(|| Rc::new(markdown::parse(text))).clone()
}

pub(super) fn answer((entry, id): (u64, u64), text: &str, at: i64, meta: &Meta) -> Div {
    let blocks = blocks_of(id, text, &meta.blocks);
    let head = div()
        .flex()
        .items_center()
        .gap(px(4.))
        .text_size(px(11.))
        .text_color(colours::faint())
        .child(div().font_weight(FontWeight::SEMIBOLD).child(format!("{} · {}", meta.name, time_label(at, meta.now))))
        .child(ui::copy_button(("bot-answer", id), Clip::of(text.to_string(), "the answer")));
    let picks = |at: usize, shown: &str| {
        let part = u32::try_from(at).ok()?;
        Some(meta.pick(Spot { entry, part }, shown))
    };
    let drawn = md::picked_blocks(&format!("bot-{id}"), &blocks, &meta.on_link, &picks);
    let body = div().flex().flex_col().gap(px(6.)).text_color(colours::soft()).children(drawn);
    div().flex().flex_col().gap(px(2.)).child(head).child(body)
}

pub(super) fn steps(first: u64, texts: &[String], kept: usize, meta: &Meta) -> Div {
    let (hidden, shown) = folded(texts, meta.opened.contains(&first), kept);
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
    div()
        .flex()
        .flex_col()
        .text_size(px(11.5))
        .text_color(colours::faint())
        .font_family(FONT_FAMILY)
        .children(more)
        .children(rows)
}

pub(super) fn error_with((spot, text): (Spot, &str), button: Option<AnyElement>, meta: &Meta) -> Div {
    let text = div().text_color(colours::failed()).child(meta.picked(spot, text));
    div().flex().flex_col().items_start().gap(px(6.)).child(text).children(button)
}

fn failed(id: u64, text: &str, meta: &Meta) -> Div {
    let retry = meta.retry.as_ref().filter(|retry| retry.error == id).map(|retry| retry_button(retry, meta));
    line(error_with((Spot { entry: id, part: 0 }, text), retry, meta))
}

fn retry_button(retry: &Retry, meta: &Meta) -> AnyElement {
    if meta.sending {
        let busy = if retry.again == Again::Setup { SETTING_UP } else { SENDING };
        return ui::busy_button("bot-retry", busy, Tone::Plain).into_any_element();
    }
    let (bot, again) = (meta.bot.clone(), retry.again.clone());
    press::stop_at(ui::button(RETRY_STOP, RETRY), &meta.retry_stop, RETRY_STOP, meta.stops)
        .on_click(move |event: &ClickEvent, _, cx| {
            if press::pressed(event, RETRY_STOP) {
                send_again(&bot, &again, cx);
            }
        })
        .into_any_element()
}

fn send_again(bot: &BotKey, again: &Again, cx: &mut App) {
    Store::get(cx).update(cx, |store, cx| match again {
        Again::Say(text) => store.say_to_bot(bot, text.clone(), Said::Again, cx),
        Again::Setup => store.set_up_bot(bot, cx),
    });
}

fn notice(id: u64, text: &str, meta: &Meta) -> Div {
    let after = meta.givable.get(&id).and_then(|ask| give_part(*ask, meta));
    let row = div().flex().flex_wrap().justify_center().items_center().gap_x(px(8.)).gap_y(px(4.));
    line(row.text_size(px(11.5)).text_color(colours::muted()).text_center().child(text.to_string()).children(after))
}

fn give_part(ask: u64, meta: &Meta) -> Option<AnyElement> {
    match &meta.giving {
        Give::Offered => Some(give_button(ask, meta).into_any_element()),
        Give::Here(words) => Some(div().text_color(colours::faint()).child(words.clone()).into_any_element()),
        Give::No => None,
    }
}

fn give_button(ask: u64, meta: &Meta) -> gpui::Stateful<Div> {
    let name = format!("bot-give-{ask}");
    let bot = meta.bot.clone();
    press::stop(ui::button(("bot-give", ask), GIVE), name.clone(), meta.stops).on_click(
        move |event: &ClickEvent, _, cx| {
            if press::pressed(event, &name) {
                Store::get(cx).update(cx, |store, cx| store.open_ask(&bot.id, ask, cx));
            }
        },
    )
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
